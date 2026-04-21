use std::time::Duration;

pub mod prometheus;
pub use self::prometheus::Prometheus;

#[allow(unused)]
pub trait MetricsAdapter {
    fn record_db_acquire_duration(&self, kind: &str, duration: Duration) {}
    fn record_db_idle_connections(&self, kind: &str, connections: u32) {}
}
