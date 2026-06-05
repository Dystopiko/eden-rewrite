use error_stack::Report;
use tracing::error;

use super::{FailedJobError, TestHarness};

impl TestHarness {
    /// Drains and runs all pending background jobs, then asserts none failed.
    ///
    /// Returns [`FailedJobError`] if any job completed with a failed status.
    ///
    /// # Panics
    ///
    /// Panics if [`TestHarnessBuilder::with_runner`] was not called.
    pub async fn run_pending_jobs(&self) -> Result<(), Report<FailedJobError>> {
        self.runner
            .as_ref()
            .expect("runner not initialized; call with_runner() on the builder")
            .start()
            .shutdown()
            .await;

        self.assert_no_failed_jobs().await
    }

    /// Asserts that the `background_jobs` table contains no pending rows.
    ///
    /// Panics with a count if any are found.
    pub async fn assert_no_pending_jobs(&self) {
        let jobs = get_all_pending_jobs(self).await;
        let count = jobs.iter().filter(|v| v.is_some()).count();
        if count > 0 {
            error!(?jobs, "found {count} pending background job(s)");
        }
        assert_eq!(count, 0, "expected no pending jobs; found {count}");
    }

    async fn assert_no_failed_jobs(&self) -> Result<(), Report<FailedJobError>> {
        let failed = fetch_failed_jobs(self).await;
        if !failed.is_empty() {
            error!(jobs = ?failed, "found {} failed background job(s)", failed.len());
            return Err(Report::new(FailedJobError));
        }
        Ok(())
    }
}

async fn get_all_pending_jobs(harness: &TestHarness) -> Vec<Option<serde_json::Value>> {
    let mut conn = harness
        .app
        .pools()
        .primary_db()
        .acquire()
        .await
        .expect("could not acquire connection");

    sqlx::query_scalar::<_, Option<serde_json::Value>>(
        "SELECT json_agg(background_jobs) FROM background_jobs",
    )
    .fetch_all(&mut *conn)
    .await
    .expect("failed to query pending jobs")
}

async fn fetch_failed_jobs(harness: &TestHarness) -> Vec<serde_json::Value> {
    let mut conn = harness
        .app
        .pools()
        .primary_db()
        .acquire()
        .await
        .expect("could not acquire connection");

    sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT data FROM background_jobs WHERE status = 'failed'",
    )
    .fetch_all(&mut *conn)
    .await
    .expect("failed to query failed jobs")
}
