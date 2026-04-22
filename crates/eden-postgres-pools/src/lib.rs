use bon::Builder;
use eden_metrics::MetricsAdapter;
use eden_postgres::{PoolError, PooledConnection, Transaction};
use error_stack::Report;
use std::{fmt, sync::Arc, time::Instant};

#[derive(Builder, Clone)]
pub struct DatabasePools {
    primary: eden_postgres::Pool,
    replica: Option<eden_postgres::Pool>,
    metrics: Option<Arc<dyn MetricsAdapter>>,
}

impl DatabasePools {
    /// Returns a reference to the primary database connection pool.
    ///
    /// The primary pool is used for all write operations and as a fallback
    /// when the replica is unavailable or unhealthy.
    #[must_use]
    pub fn primary(&self) -> &eden_postgres::Pool {
        &self.primary
    }

    /// Returns a reference to the replica database connection pool, if one is configured.
    ///
    /// The replica pool is used for read operations when available and healthy.
    /// Returns `None` if no replica has been configured, in which case reads
    /// will fall back to the primary database.
    #[must_use]
    pub fn replica(&self) -> Option<&eden_postgres::Pool> {
        self.replica.as_ref()
    }
}

impl DatabasePools {
    /// Acquires a write connection from the primary database as a transaction.
    ///
    /// This should be used for any operations that modify the database. It always
    /// targets the primary pool. Replicas are never used for writes.
    #[tracing::instrument(skip_all, name = "db.write")]
    pub async fn write(&self) -> Result<Transaction<'static>, Report<PoolError>> {
        tracing::debug!("obtaining primary database connection...");

        let start = Instant::now();
        let result = self.primary.begin().await;
        if let Some(metrics) = self.metrics.as_ref() {
            metrics.record_db_acquire_duration("primary", start.elapsed());
        }

        result
    }

    /// Acquires a read connection, preferring the replica database if available.
    ///
    /// Connection selection follows this priority order:
    /// 1. **Replica** — used if configured and healthy.
    /// 2. **Primary** — used as a fallback if no replica is configured, or if
    ///    the replica reports itself as [`PoolError::Unhealthy`].
    ///
    /// This method is suitable for the majority of read-only queries in a
    /// primary/replica setup, since it offloads read traffic to the replica
    /// whenever possible.
    #[tracing::instrument(skip_all, name = "db.read")]
    pub async fn read(&self) -> Result<PooledConnection, Report<PoolError>> {
        let Some(replica) = self.replica.as_ref() else {
            tracing::debug!("obtaining primary database connection...");
            return self.acquire_from_primary().await;
        };

        tracing::debug!("obtaining replica database connection...");

        let start = Instant::now();
        let result = match replica.acquire().await {
            Ok(conn) => Ok(conn),
            Err(error) if matches!(error.current_context(), PoolError::Unhealthy) => {
                tracing::warn!(
                    ?error,
                    "replica database is unhealthy, falling back to primary"
                );
                self.acquire_from_primary().await
            }
            Err(error) => Err(error),
        };

        if let Some(metrics) = self.metrics.as_ref() {
            metrics.record_db_acquire_duration("replica", start.elapsed());
        }

        result
    }

    /// Acquires a read connection, preferring the primary database over the replica.
    ///
    /// Connection selection follows this priority order:
    /// 1. **Primary** — always attempted first.
    /// 2. **Replica** — used as a fallback only if the primary reports itself as
    ///    [`PoolError::Unhealthy`] and a replica is configured.
    ///
    /// This is useful for read operations that require the most up-to-date data,
    /// such as reads that immediately follow a write, where replica lag would be
    /// unacceptable. Prefer [`db_read`] for general-purpose reads to reduce load
    /// on the primary.
    ///
    /// [`db_read`]: DatabasePools::db_read
    #[tracing::instrument(skip_all, name = "db.read_prefer_primary")]
    pub async fn read_prefer_primary(&self) -> Result<PooledConnection, Report<PoolError>> {
        tracing::debug!("obtaining primary database connection...");

        let error = match self.acquire_from_primary().await {
            Ok(conn) => return Ok(conn),
            Err(error) => error,
        };

        if !matches!(error.current_context(), PoolError::Unhealthy) {
            return Err(error);
        }

        let Some(replica) = self.replica.as_ref() else {
            return Err(error);
        };

        tracing::warn!(
            ?error,
            "replica database is unhealthy, falling back to primary"
        );

        let start = Instant::now();
        let result = replica.acquire().await;
        if let Some(metrics) = self.metrics.as_ref() {
            metrics.record_db_acquire_duration("replica", start.elapsed());
        }
        result
    }
}

impl DatabasePools {
    async fn acquire_from_primary(&self) -> Result<PooledConnection, Report<PoolError>> {
        let start = Instant::now();
        let result = self.primary().acquire().await;
        if let Some(metrics) = self.metrics.as_ref() {
            metrics.record_db_acquire_duration("primary", start.elapsed());
        }
        result
    }
}

impl fmt::Debug for DatabasePools {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DatabasePools")
            .field("primary", &self.primary)
            .field("replica", &self.replica)
            .finish()
    }
}

#[cfg(test)]
mod tests;
