use std::{fmt, time::Duration};

pub mod prometheus;
use erased_report::ErasedReport;

pub use self::prometheus::Prometheus;

#[mockall::automock]
#[allow(unused)]
pub trait MetricsAdapter: fmt::Debug + Send + Sync {
    fn encode_to_http(&self) -> Result<String, ErasedReport>;
    fn record_db_acquire_duration(&self, kind: &str, duration: Duration) {}
    fn record_db_idle_connections(&self, kind: &str, connections: u32) {}
}
