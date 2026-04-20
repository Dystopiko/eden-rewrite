use doku::Document;
use eden_config_derive::Validate;
use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr};

pub mod tls;
pub mod validators;

pub use self::tls::Tls;

#[derive(Clone, Debug, Deserialize, Document, Eq, PartialEq, Validate)]
#[serde(default)]
pub struct Gateway {
    #[validate(skip)]
    pub ip: IpAddr,
    #[validate(skip)]
    pub port: u16,

    /// Enable or disable Prometheus metrics collection.
    ///
    /// When enabled, metrics are exposed at the `/metrics` endpoint.
    ///
    /// Available metrics:
    /// - `database_idle_conns` - Number of idle database connections
    /// - `database_used_conns` - Number of active database connections
    /// - `database_time_to_acquire_connection` - Time spent waiting for a connection
    /// - `events_processed` - Total Discord gateway events processed
    /// - `requests_total` - Total HTTP requests received
    /// - `response_times` - HTTP response time distribution
    /// - `shard_latencies` - Per-shard gateway latency
    /// - `sessions_granted` - Total sessions granted to players
    #[validate(skip)]
    pub metrics: bool,

    pub tls: Option<Tls>,
}

impl Gateway {
    pub const DEFAULT_IP: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

    // Inspired from a popular nature park in the Philippines
    pub const DEFAULT_PORT: u16 = 7590;
}

impl Default for Gateway {
    fn default() -> Self {
        Self {
            ip: Self::DEFAULT_IP,
            port: Self::DEFAULT_PORT,
            metrics: true,
            tls: None,
        }
    }
}
