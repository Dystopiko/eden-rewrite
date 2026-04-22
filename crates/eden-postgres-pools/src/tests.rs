use dashmap::DashMap;
use eden_config::types::database::{Common, DatabasePool};
use eden_metrics::MetricsAdapter;
use eden_postgres::Pool;
use std::{sync::Arc, time::Duration};

use crate::DatabasePools;

#[tokio::test]
async fn should_fallback_to_replica_if_primary_is_unhealthy() {
    eden_test_util::init_tracing_for_tests();

    let non_existent_db_config = DatabasePool::builder()
        .url("postgres://127.0.0.2:1".to_string().into())
        .build();

    let replica = Pool::new_for_tests().build().await;
    let primary = Pool::new(
        Common::builder()
            .connect_timeout(Duration::from_millis(10))
            .build(),
        non_existent_db_config,
    )
    .unwrap();

    let metrics: Arc<MetricsCollector> = Arc::new(MetricsCollector::new());
    let pools = DatabasePools::builder()
        .primary(primary)
        .replica(replica)
        .metrics(metrics.clone())
        .build();

    // primary label should have collected one duration
    if let Err(error) = pools.read_prefer_primary().await {
        panic!("should have fallen back to primary: {error:#?}");
    }

    assert_eq!(metrics.acquire_times.get("replica").unwrap().len(), 1);
}

#[tokio::test]
async fn should_fallback_to_primary_if_replica_is_unhealthy() {
    eden_test_util::init_tracing_for_tests();

    let non_existent_db_config = DatabasePool::builder()
        .url("postgres://127.0.0.2:1".to_string().into())
        .build();

    let primary = Pool::new_for_tests().build().await;
    let replica = Pool::new(
        Common::builder()
            .connect_timeout(Duration::from_millis(10))
            .build(),
        non_existent_db_config,
    )
    .unwrap();

    let metrics: Arc<MetricsCollector> = Arc::new(MetricsCollector::new());
    let pools = DatabasePools::builder()
        .primary(primary)
        .replica(replica)
        .metrics(metrics.clone())
        .build();

    // primary label should have collected one duration
    if let Err(error) = pools.read().await {
        panic!("should have fallen back to primary: {error:#?}");
    }

    assert_eq!(metrics.acquire_times.get("primary").unwrap().len(), 1);
}

#[tokio::test]
async fn should_collect_metrics() {
    let (pools, metrics) = init_healthy_pools().await;
    eden_test_util::init_tracing_for_tests();

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

async fn init_healthy_pools() -> (DatabasePools, Arc<MetricsCollector>) {
    let metrics: Arc<MetricsCollector> = Arc::new(MetricsCollector::new());
    let pools = DatabasePools::builder()
        .primary(Pool::new_for_tests().build().await)
        .replica(Pool::new_for_tests().build().await)
        .metrics(metrics.clone())
        .build();

    (pools, metrics)
}

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
    fn record_db_acquire_duration(&self, kind: &str, duration: Duration) {
        self.acquire_times
            .entry(kind.to_string())
            .or_default()
            .push(duration);
    }
}
