use bon::Builder;
use eden_metrics::MetricsAdapter;
use eden_postgres::{PoolError, PooledConnection, Transaction};
use error_stack::Report;
use std::sync::Arc;
use std::time::Instant;

/// Manages primary and optional replica PostgreSQL connection pools.
#[derive(Builder, Clone, Debug)]
pub struct DatabasePools {
    primary_db: eden_postgres::Pool,
    replica_db: Option<eden_postgres::Pool>,
    metrics: Option<Arc<dyn MetricsAdapter>>,
}

impl DatabasePools {
    /// Returns a reference to the primary database connection pool.
    ///
    /// The primary pool is used for all write operations and as a fallback
    /// when the replica is unavailable or unhealthy.
    #[must_use]
    pub fn primary_db(&self) -> &eden_postgres::Pool {
        &self.primary_db
    }

    /// Returns a reference to the replica database connection pool, if one is configured.
    ///
    /// The replica pool is used for read operations when available and healthy.
    /// Returns `None` if no replica has been configured, in which case reads
    /// will fall back to the primary database.
    #[must_use]
    pub fn replica_db(&self) -> Option<&eden_postgres::Pool> {
        self.replica_db.as_ref()
    }

    /// Acquires a write connection from the primary database as a transaction.
    ///
    /// This should be used for any operations that modify the database. It always
    /// targets the primary pool — replicas are never used for writes.
    #[tracing::instrument(skip_all, name = "db.write")]
    pub async fn write(&self) -> Result<Transaction<'static>, Report<PoolError>> {
        tracing::debug!("obtaining primary database connection...");

        let start = Instant::now();
        let conn = self.primary_db().begin().await;
        if let Some(metrics) = self.metrics.as_ref() {
            metrics.record_db_acquire_duration("primary", start.elapsed());
        }

        conn
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
        let replica_db = self.replica_db();
        let Some(replica) = replica_db.as_ref() else {
            tracing::debug!("obtaining primary database connection...");
            return self.acquire_from_primary().await;
        };

        tracing::debug!("obtaining replica database connection...");

        let start = Instant::now();
        let result = match replica.acquire().await {
            Ok(conn) => Ok(conn),
            Err(error) => match error.current_context() {
                PoolError::Unhealthy => {
                    tracing::warn!(
                        ?error,
                        "replica database is unhealthy, falling back to primary"
                    );
                    self.acquire_from_primary().await
                }
                _ => Err(error),
            },
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
    /// unacceptable. Prefer [`read`] for general-purpose reads to reduce load
    /// on the primary.
    ///
    /// [`read`]: DatabasePools::read
    #[tracing::instrument(skip_all, name = "db.read_prefer_primary")]
    pub async fn read_prefer_primary(&self) -> Result<PooledConnection, Report<PoolError>> {
        tracing::debug!("obtaining primary database connection...");

        let error = match self.acquire_from_primary().await {
            Ok(conn) => return Ok(conn),
            Err(error) => error,
        };

        if let PoolError::Unhealthy = error.current_context()
            && let Some(replica) = self.replica_db().as_ref()
        {
            tracing::warn!(
                ?error,
                "primary database is unhealthy, falling back to replica"
            );

            let start = Instant::now();
            let result = replica.acquire().await;

            if let Some(metrics) = self.metrics.as_ref() {
                metrics.record_db_acquire_duration("replica", start.elapsed());
            }

            return result;
        }

        Err(error)
    }
}

impl DatabasePools {
    async fn acquire_from_primary(&self) -> Result<PooledConnection, Report<PoolError>> {
        let start = Instant::now();
        let conn = self.primary_db().acquire().await;
        if let Some(metrics) = self.metrics.as_ref() {
            metrics.record_db_acquire_duration("primary", start.elapsed());
        }

        conn
    }
}

#[cfg(test)]
mod tests {
    use dashmap::DashMap;
    use eden_config::types::database::{Common, DatabasePool};
    use eden_metrics::MetricsAdapter;
    use eden_postgres::Pool;
    use erased_report::ErasedReport;
    use std::{sync::Arc, time::Duration};

    use crate::DatabasePools;

    #[sqlx::test]
    async fn should_fallback_to_replica_if_primary_is_unhealthy(pool: sqlx::PgPool) {
        eden_test_util::init_tracing_for_tests();

        let common = Common::builder()
            .connect_timeout(Duration::from_millis(100))
            .statement_timeout(Duration::from_millis(100))
            .build();

        let unhealthy_pool = unhealthy_pool(&common);
        let metrics = Arc::new(MetricsCollector::new());

        let pools = DatabasePools::builder()
            .primary_db(unhealthy_pool)
            .replica_db(pool.into())
            .metrics(metrics.clone())
            .build();

        // replica label should have collected one duration
        if let Err(error) = pools.read_prefer_primary().await {
            panic!("should have fallen back to replica: {error:#?}");
        }

        assert_eq!(metrics.acquire_times.get("replica").unwrap().len(), 1);
    }

    #[sqlx::test]
    async fn should_fallback_to_primary_if_replica_is_unhealthy(pool: sqlx::PgPool) {
        eden_test_util::init_tracing_for_tests();

        let common = Common::builder()
            .connect_timeout(Duration::from_millis(100))
            .statement_timeout(Duration::from_millis(100))
            .build();

        let unhealthy_pool = unhealthy_pool(&common);
        let metrics = Arc::new(MetricsCollector::new());

        let pools = DatabasePools::builder()
            .primary_db(pool.into())
            .replica_db(unhealthy_pool)
            .metrics(metrics.clone())
            .build();

        // primary label should have collected one duration
        if let Err(error) = pools.read().await {
            panic!("should have fallen back to primary: {error:#?}");
        }

        assert_eq!(metrics.acquire_times.get("replica").unwrap().len(), 1);
    }

    #[sqlx::test]
    async fn should_collect_metrics(pool: sqlx::PgPool) {
        eden_test_util::init_tracing_for_tests();

        let metrics = Arc::new(MetricsCollector::new());
        let pools = DatabasePools::builder()
            .primary_db(pool.clone().into())
            .replica_db(pool.into())
            .metrics(metrics.clone())
            .build();

        // replica label should have collected one duration
        pools.read().await.unwrap();
        assert_eq!(metrics.acquire_times.get("replica").unwrap().len(), 1);

        // primary label should have collected one duration
        pools.read_prefer_primary().await.unwrap();
        assert_eq!(metrics.acquire_times.get("primary").unwrap().len(), 1);

        // primary label should have collected two durations
        pools.write().await.unwrap();
        assert_eq!(metrics.acquire_times.get("primary").unwrap().len(), 2);
    }

    fn unhealthy_pool(common: &Common) -> Pool {
        let config = DatabasePool::builder()
            .url("postgres://127.0.0.2".to_string().into())
            .build();

        Pool::new(common.clone(), config).unwrap()
    }

    #[derive(Debug)]
    struct MetricsCollector {
        acquire_times: DashMap<String, Vec<Duration>>,
    }

    impl MetricsCollector {
        fn new() -> Self {
            Self {
                acquire_times: DashMap::new(),
            }
        }
    }

    impl MetricsAdapter for MetricsCollector {
        fn encode_to_http(&self) -> Result<String, ErasedReport> {
            unreachable!()
        }

        fn record_db_acquire_duration(&self, kind: &str, duration: Duration) {
            self.acquire_times
                .entry(kind.to_string())
                .or_default()
                .push(duration);
        }
    }
}
