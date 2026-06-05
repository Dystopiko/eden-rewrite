use axum_test::TestServer;
use eden_api_server::WebContext;
use eden_background_worker::runner::Runner;
use eden_common::AppContext;
use eden_jobs::JobContext;
use std::{ops, sync::Arc};
use thiserror::Error;

pub mod builders;
#[allow(unused)]
pub mod mock_user;

pub use self::builders::{OrganizationSetup, TestHarnessBuilder};
pub use self::mock_user::MockUser;

mod background_jobs;
mod db;
mod fixtures;

/// The root test harness. Wraps an [`AppContext`] and an optional background
/// job runner for use in integration tests.
///
/// Construct via [`TestHarness::builder`]. Derefs to [`AppContext`], so all
/// pool, config, and service accessors are available directly on the harness.
pub struct TestHarness {
    pub(crate) app: Arc<AppContext>,
    pub(crate) runner: Option<Runner<Arc<JobContext>>>,
}

impl TestHarness {
    /// Returns a builder for constructing a [`TestHarness`] backed by the
    /// given database pool.
    pub fn builder(pool: impl Into<eden_postgres::Pool>) -> TestHarnessBuilder {
        TestHarnessBuilder::new(pool.into())
    }

    /// Returns a clone of the underlying [`AppContext`].
    #[must_use]
    pub fn context(&self) -> Arc<AppContext> {
        self.app.clone()
    }

    /// Runs all pending database migrations against the primary pool.
    ///
    /// Call this once at the start of each test that depends on schema
    /// being present.
    pub async fn run_migrations(&self) {
        eden_model::tables::migrations::perform(self.pools().primary_db())
            .await
            .expect("migrations failed");
    }

    /// Builds a [`TestServer`] wired to the API router over a mock transport.
    ///
    /// Each call produces an independent server instance. There is no shared
    /// state between calls.
    pub fn server(&self) -> TestServer {
        let ctx = Arc::new(WebContext {
            app: self.app.clone(),
        });

        let router = eden_api_server::router::build(ctx);
        TestServer::builder()
            .do_not_save_cookies()
            .mock_transport()
            .build(router)
    }
}

impl ops::Deref for TestHarness {
    type Target = AppContext;

    fn deref(&self) -> &Self::Target {
        &self.app
    }
}

/// Returned by [`TestHarness::run_pending_jobs`] when one or more
/// background jobs completed with a failed status.
#[derive(Debug, Error)]
#[error("encountered failed background jobs")]
pub struct FailedJobError;
