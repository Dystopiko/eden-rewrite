use eden_config::types::database::{Common, DatabasePool as Config};
use rand::RngExt;
use sqlx::postgres::PgConnectOptions;
use std::{str::FromStr, time::Duration};

/// Builder for creating test database pools.
///
/// This builder provides a fluent interface for configuring test pools
/// with custom settings before creation.
///
/// # Examples
///
/// ```no_run
/// # use eden_postgres::Pool;
/// # use std::time::Duration;
/// # async fn example() {
/// let pool = Pool::new_for_tests()
///     .max_connections(50.try_into().unwrap())
///     .readonly(true)
///     .connect_timeout(Duration::from_secs(10))
///     .build()
///     .await;
/// # }
/// ```
#[derive(Debug)]
pub struct TestPoolBuilder {
    common: Common,
    min_connections: u32,
    max_connections: Option<u32>,
    readonly: bool,
}

impl TestPoolBuilder {
    /// Creates a new test pool builder with default values.
    pub(super) fn new() -> Self {
        Self {
            common: Common::default(),
            min_connections: 0,
            max_connections: Some(100),
            readonly: false,
        }
    }

    /// Sets the maximum number of connections in the pool.
    ///
    /// # Panics
    ///
    /// Panics if `max` is zero.
    #[must_use]
    pub fn max_connections(mut self, max: std::num::NonZeroU32) -> Self {
        self.max_connections = Some(max.get());
        self
    }

    /// Sets the minimum number of connections to maintain in the pool.
    #[must_use]
    pub fn min_connections(mut self, min: u32) -> Self {
        self.min_connections = min;
        self
    }

    /// Sets whether the pool should enforce readonly mode.
    ///
    /// When enabled, all transactions will be readonly by default.
    #[must_use]
    pub fn readonly(mut self, readonly: bool) -> Self {
        self.readonly = readonly;
        self
    }

    /// Sets the connection timeout.
    ///
    /// This is the maximum time to wait when establishing a new connection.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.common.connect_timeout = timeout;
        self
    }

    /// Sets the statement timeout.
    ///
    /// This is the maximum time allowed for a single SQL statement to execute.
    #[must_use]
    pub fn statement_timeout(mut self, timeout: Duration) -> Self {
        self.common.statement_timeout = timeout;
        self
    }

    /// Builds the test pool with the configured settings.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - The embedded PostgreSQL server fails to start
    /// - The test database cannot be created
    /// - The connection URL is invalid (should never happen)
    #[must_use]
    pub async fn build(self) -> crate::Pool {
        let server = crate::embedded::start_for_tests().await;
        let generated_name = format!("test_{}", generate_name());

        server
            .create_database(&generated_name)
            .await
            .expect("Failed to create test database");

        let url = PgConnectOptions::from_str(&server.settings().url(&generated_name))
            .expect("Embedded server should generate valid PostgreSQL connection URL");

        let max_connections = self
            .max_connections
            .and_then(std::num::NonZeroU32::new)
            .expect("max_connections must be greater than zero");

        let config = Config::builder()
            .empty_url()
            .min_connections(self.min_connections)
            .max_connections(max_connections)
            .readonly(self.readonly)
            .build();

        crate::Pool::from_inner(url, self.common, config)
    }
}

fn generate_name() -> String {
    let mut rng = rand::rng();
    std::iter::repeat(())
        .map(|_| rng.sample(rand::distr::Alphanumeric) as char)
        .take(20)
        .collect()
}
