//! PostgreSQL connection pool management.
//!
//! This module provides a type-safe wrapper around `sqlx::PgPool` with additional
//! functionality for testing, health checks, and connection lifecycle management.

use crate::error::PgErrorType;
use eden_config::types::database::{Common, DatabasePool as Config};
use error_stack::{Report, ResultExt};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{str::FromStr, time::Duration};

mod error;

pub use self::error::{InvalidConnectionUrl, PoolError};
pub use sqlx::PgConnection as Connection;

/// A borrowed transaction from the PostgreSQL pool.
///
/// Transactions provide ACID guarantees for a series of database operations.
/// They are automatically rolled back when dropped unless explicitly committed
/// via [`Transaction::commit`].
pub type Transaction<'q> = sqlx::Transaction<'q, sqlx::Postgres>;

/// A connection checked out from the pool.
///
/// The connection is automatically returned to the pool when dropped.
/// Holding a connection prevents other tasks from using it, so it's
/// best to acquire connections only when needed and release them quickly.
pub type PooledConnection = sqlx::pool::PoolConnection<sqlx::Postgres>;

/// An asynchronous pool of PostgreSQL database connections.
///
/// This object is a pointer of [`sqlx::PgPool`].
#[derive(Clone)]
pub struct Pool {
    inner: sqlx::PgPool,
}

impl Pool {
    /// Creates a new connection pool from configuration.
    ///
    /// The pool is created lazily, meaning no connections are established until
    /// the first operation is performed. This makes pool creation fast and allows
    /// the application to start even if the database is temporarily unavailable.
    #[track_caller]
    pub fn new(common: Common, config: Config) -> Result<Self, Report<InvalidConnectionUrl>> {
        let url = PgConnectOptions::from_str(&config.url).change_context(InvalidConnectionUrl)?;
        Ok(Self::from_inner(url, common, config))
    }

    /// Returns a reference to the underlying [`sqlx::PgPool`].
    ///
    /// This provides direct access to the `sqlx` pool for cases where you need
    /// functionality not exposed by the `Pool` wrapper.
    #[must_use]
    pub fn inner(&self) -> &sqlx::PgPool {
        &self.inner
    }
}

impl Pool {
    /// Acquires a connection from the pool.
    ///
    /// This function will wait if all connections in the pool are currently in use.
    /// The connection is automatically returned to the pool when dropped.
    pub async fn acquire(&self) -> Result<PooledConnection, Report<PoolError>> {
        self.inner.acquire().await.map_err(PoolError::from_sqlx)
    }

    /// Begins a new database transaction.
    ///
    /// The transaction acquires a dedicated connection from the pool and holds it
    /// until the transaction is committed or rolled back.
    pub async fn begin(&self) -> Result<Transaction<'static>, Report<PoolError>> {
        self.inner.begin().await.map_err(PoolError::from_sqlx)
    }

    /// Checks the health of the database connection pool.
    ///
    /// This function performs a lightweight probe to determine if the pool can
    /// successfully acquire a connection and execute a trivial query (`SELECT 1`).
    pub async fn check_health(&self, timeout: Option<Duration>) -> Result<bool, Report<PoolError>> {
        let timeout = timeout.unwrap_or(Duration::from_secs(5));
        tokio::time::timeout(timeout, self.probe())
            .await
            .unwrap_or(Ok(false))
    }

    /// Runs a lightweight probe query against a freshly acquired connection.
    ///
    /// This is an internal helper function separated from [`Pool::check_health`]
    /// to keep the timeout logic and the actual health check logic separate.
    async fn probe(&self) -> Result<bool, Report<PoolError>> {
        use PgErrorType::UnhealthyConnection;

        let mut conn = match self.inner.acquire().await {
            Ok(conn) => conn,
            Err(error) if matches!(PgErrorType::from_sqlx(&error), UnhealthyConnection) => {
                return Ok(false);
            }
            Err(error) => return Err(error).change_context(PoolError::General),
        };

        match sqlx::query("SELECT 1").execute(&mut *conn).await {
            Ok(..) => Ok(true),
            Err(error) if matches!(PgErrorType::from_sqlx(&error), UnhealthyConnection) => {
                Ok(false)
            }
            Err(error) => Err(error).change_context(PoolError::General),
        }
    }
}

impl std::fmt::Debug for Pool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.inner, f)
    }
}

impl From<sqlx::PgPool> for Pool {
    fn from(inner: sqlx::PgPool) -> Self {
        Self { inner }
    }
}

impl Pool {
    fn from_inner(url: sqlx::postgres::PgConnectOptions, common: Common, config: Config) -> Self {
        let inner = PgPoolOptions::new()
            .min_connections(config.min_connections)
            .max_connections(config.max_connections.get())
            .acquire_timeout(common.connect_timeout)
            .test_before_acquire(true)
            .after_connect(move |conn, _metadata| {
                Box::pin(setup_pg_connection(
                    conn,
                    config.readonly,
                    common.statement_timeout,
                ))
            })
            .connect_lazy_with(url);

        Self { inner }
    }
}

async fn setup_pg_connection(
    conn: &mut sqlx::PgConnection,
    readonly_mode: bool,
    statement_timeout: Duration,
) -> sqlx::Result<()> {
    sqlx::query("SET application_name = 'eden'")
        .execute(&mut *conn)
        .await?;

    if readonly_mode {
        sqlx::query("SET default_transaction_read_only = 't'")
            .execute(&mut *conn)
            .await?;
    }

    let timeout = statement_timeout.as_millis();
    sqlx::query(&format!("SET statement_timeout = {timeout}"))
        .execute(conn)
        .await?;

    Ok(())
}
