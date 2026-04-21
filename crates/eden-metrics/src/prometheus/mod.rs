mod macros;

use self::macros::metrics;

use prometheus::{HistogramVec, IntGaugeVec};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("Failed to encode prometheus metrics into string")]
pub struct EncodeError;

metrics! {
    pub struct Prometheus {
        "eden_database" => {
            /// Number of idle database connections in the poo
            pub database_idle_conns: IntGaugeVec["pool"],

            /// Number of used database connections in the pool
            pub database_used_conns: IntGaugeVec["pool"],

            /// Total time required to acquire a database connection
            pub database_time_to_acquire_connection: HistogramVec["pool"],
        },
    }
}

impl crate::MetricsAdapter for Prometheus {
    fn record_db_acquire_duration(&self, kind: &str, duration: std::time::Duration) {
        self.database_time_to_acquire_connection
            .get_metric_with_label_values(&[kind])
            .expect("should only require one label")
            .observe(duration.as_secs_f64());
    }

    fn record_db_idle_connections(&self, kind: &str, connections: u32) {
        self.database_idle_conns
            .get_metric_with_label_values(&[kind])
            .expect("should only require one label")
            .set(connections as i64);
    }
}
