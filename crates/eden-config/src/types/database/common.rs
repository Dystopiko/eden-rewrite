use bon::Builder;
use doku::Document;
use serde::Deserialize;
use std::time::Duration;

#[derive(Builder, Clone, Debug, Deserialize, Document, Eq, PartialEq)]
#[serde(default)]
pub struct Common {
    /// Maximum time to wait when establishing a new database connection.
    ///
    /// If the database server doesn't respond within this time, the connection
    /// attempt will fail with a timeout error.
    ///
    /// It defaults to 5 seconds if not set.
    #[builder(default = Duration::from_secs(5))]
    pub connect_timeout: Duration,

    /// Maximum time allowed for a single SQL statement to execute.
    ///
    /// If a query takes longer than this duration, it will be cancelled by the
    /// database server. This helps prevent runaway queries from consuming
    /// resources indefinitely.
    ///
    /// It defaults to 15 seconds if not set.
    #[builder(default = Duration::from_secs(15))]
    pub statement_timeout: Duration,
}

impl Default for Common {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            statement_timeout: Duration::from_secs(15),
        }
    }
}
