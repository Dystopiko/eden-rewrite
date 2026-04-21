use eden_config::types::database::DatabasePool as Config;
use error_stack::{Report, ResultExt};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{str::FromStr, time::Duration};
use thiserror::Error;

pub use sqlx::SqliteConnection as Connection;

use crate::error::SqliteErrorType;

/// A borrowed transaction on the SQLite pool.
///
/// They are automatically rolled back when dropped unless explicitly committed.
pub type Transaction<'q> = sqlx::Transaction<'q, sqlx::Sqlite>;

/// A connection checked out from the pool. Returned to the pool on drop.
pub type PooledConnection = sqlx::pool::PoolConnection<sqlx::Sqlite>;

/// An asynchronous pool of database connections.
///
/// This object is a pointer of [`sqlx::SqlitePool`].
#[derive(Clone)]
pub struct Pool {
    inner: sqlx::SqlitePool,
}

impl Pool {
    /// Creates a new pool from the given [`pool configuration`].
    ///
    /// The pool is created lazily so no connections are opened until
    /// the first operation is performed.
    ///
    /// [`pool configuration`]: eden_config::types::database::DatabasePool
    pub fn new(config: Config) -> Result<Self, Report<InvalidConnectionUrl>> {
        let url = SqliteConnectOptions::from_str(config.url.as_str())
            .change_context(InvalidConnectionUrl)?
            .read_only(config.readonly);

        let inner = SqlitePoolOptions::new()
            .min_connections(config.min_connections)
            .max_connections(config.max_connections.get())
            .test_before_acquire(true)
            .connect_lazy_with(url);

        Ok(Self { inner })
    }

    /// Opens a database pool backed by an in-memory SQLite database with
    /// optional configuration to setup minimum and maximum connections,
    /// and other options through the [`DatabasePool`] config.
    ///
    /// In-memory databases are useful for testing and temporary data storage.
    /// Each pool gets its own isolated in-memory database that exists only
    /// for the lifetime of the pool.
    ///
    /// # Default Configuration
    ///
    /// If `config` is set to `None`, these values will set to its defaults:
    ///
    /// | Field             | Value   |
    /// |-------------------|---------|
    /// | `max_connections` | `100`   |
    /// | `min_connections` | `0`     |
    /// | `readonly`        | `false` |
    ///
    /// [`DatabasePool`]: eden_config::types::database::DatabasePool
    pub fn memory(config: Option<Config>) -> Result<Self, Report<InvalidConnectionUrl>> {
        let mut url = if let Some(config) = config.as_ref() {
            SqliteConnectOptions::from_str(config.url.as_str())
                .change_context(InvalidConnectionUrl)?
        } else {
            SqliteConnectOptions::from_str(":memory:")
                .expect("should be a valid SQLite connection url")
        }
        .in_memory(true);

        let mut inner = SqlitePoolOptions::new();
        if let Some(config) = config.as_ref() {
            url = url.read_only(config.readonly);
            inner = inner
                .min_connections(config.min_connections)
                .max_connections(config.max_connections.get());
        }

        let inner = inner.connect_lazy_with(url);
        Ok(Self { inner })
    }
}

impl Pool {
    /// Acquires a connection from the pool, waiting if none are currently available.
    ///
    /// The connection is returned to the pool automatically when dropped.
    pub async fn acquire(&self) -> Result<PooledConnection, Report<PoolError>> {
        self.inner.acquire().await.map_err(PoolError::from_sqlx)
    }

    /// Begins a new database transaction, acquiring a connection from the pool.
    ///
    /// The transaction must be explicitly committed via [`Transaction::commit`];
    /// otherwise it will be rolled back on drop.
    pub async fn begin(&self) -> Result<Transaction<'static>, Report<PoolError>> {
        self.inner.begin().await.map_err(PoolError::from_sqlx)
    }

    /// Checks whether the pool can successfully acquire a connection and execute
    /// a trivial query (`SELECT 1`).
    ///
    /// Returns `true` if the probe succeeds, or `false` if the pool is unhealthy
    /// or the `timeout` elapses before the probe completes. If no timeout is
    /// provided, a default of 5 seconds is used.
    pub async fn check_health(&self, timeout: Option<Duration>) -> Result<bool, Report<PoolError>> {
        let timeout = timeout.unwrap_or(Duration::from_secs(5));
        tokio::time::timeout(timeout, self.probe())
            .await
            .unwrap_or(Ok(false))
    }

    /// Runs a lightweight probe query against a freshly acquired connection.
    ///
    /// Separated from [`Pool::check_health`] so that the timeout wrapper and
    /// the actual health logic each have a single responsibility.
    async fn probe(&self) -> Result<bool, Report<PoolError>> {
        use SqliteErrorType::UnhealthyConnection;

        let mut conn = match self.inner.acquire().await {
            Ok(conn) => conn,
            Err(error) if matches!(SqliteErrorType::from_sqlx(&error), UnhealthyConnection) => {
                return Ok(false);
            }
            Err(error) => return Err(error).change_context(PoolError::General),
        };

        match sqlx::query("SELECT 1").execute(&mut *conn).await {
            Ok(..) => Ok(true),
            Err(error) if matches!(SqliteErrorType::from_sqlx(&error), UnhealthyConnection) => {
                Ok(false)
            }
            Err(error) => Err(error).change_context(PoolError::General),
        }
    }
}

impl Pool {
    /// Returns the reference of an inner [`SqlitePool`] value.
    ///
    /// [`SqlitePool`]: sqlx::SqlitePool
    #[must_use]
    pub fn inner(&self) -> &sqlx::SqlitePool {
        &self.inner
    }
}

impl std::fmt::Debug for Pool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.inner, f)
    }
}

impl From<sqlx::SqlitePool> for Pool {
    fn from(inner: sqlx::SqlitePool) -> Self {
        Self { inner }
    }
}

/// Errors that can occur during pool operations such as acquiring a connection,
/// beginning a transaction, or checking pool health.
#[derive(Debug, Error)]
pub enum PoolError {
    /// The pool or an underlying connection is in a state where it cannot serve
    /// requests. Typically indicates a misconfigured path, a full pool, or a
    /// crashed worker. Callers may wish to surface this as a health failure.
    ///
    /// This error suggests the database layer is not healthy and may require
    /// intervention or a service restart.
    #[error("Failed to acquire pool connection")]
    General,

    /// The operation failed for a reason unrelated to pool health, such as a
    /// query error or an unexpected driver error.
    ///
    /// This error indicates a transient issue that doesn't necessarily mean
    /// the pool itself is unhealthy.
    #[error("Pool is unhealthy")]
    Unhealthy,
}

impl PoolError {
    fn from_sqlx(error: sqlx::Error) -> Report<Self> {
        let context = match SqliteErrorType::from_sqlx(&error) {
            SqliteErrorType::UnhealthyConnection => Self::Unhealthy,
            _ => Self::General,
        };
        Report::new(error).change_context(context)
    }
}

/// Error returned when a SQLite connection URL is invalid or malformed.
///
/// This error is returned during pool creation when the provided connection
/// URL cannot be parsed as a valid SQLite connection string.
#[derive(Debug, Error)]
#[error("Invalid SQLite connection URL")]
pub struct InvalidConnectionUrl;
