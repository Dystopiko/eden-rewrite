use bon::Builder;
use eden_sqlx_sqlite::{Connection, Transaction};
use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use thiserror::Error;
use uuid::Uuid;

/// Represents a background job in the database.
///
/// Background jobs are asynchronous tasks that can be queued for later execution.
/// They support priorities, retries, and various status states.
#[derive(Clone, Debug, Deserialize, Eq, FromRow, PartialEq, Serialize)]
pub struct BackgroundJob {
    pub id: Uuid,
    #[sqlx(rename = "type")]
    pub kind: String,
    pub created_at: Timestamp,
    pub data: String,
    pub last_retry: Option<Timestamp>,
    pub priority: i16,
    pub retries: i16,
    pub status: JobStatus,
}

/// Status of a background job.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum JobStatus {
    /// Job is waiting in the queue to be executed.
    Enqueued,

    /// Job is currently being executed.
    Running,

    /// Job execution failed and won't be retried.
    Failed,
}

/// Error that occurs when querying background jobs from the database.
#[derive(Debug, Error)]
#[error("Failed to query background job table from the database")]
pub struct JobQueryError;

impl BackgroundJob {
    /// Clears all background jobs from the database.
    ///
    /// It returns the number of jobs that were deleted.
    pub async fn clear(conn: &mut Transaction<'_>) -> Result<u64, Report<JobQueryError>> {
        sqlx::query("TRUNCATE TABLE background_jobs")
            .execute(&mut **conn)
            .await
            .change_context(JobQueryError)
            .attach("while trying to clear all background jobs")
            .map(|v| v.rows_affected())
    }

    /// Deletes a specific background job by ID.
    pub async fn delete(conn: &mut Connection, id: Uuid) -> Result<(), Report<JobQueryError>> {
        sqlx::query(
            "DELETE FROM background_jobs
            WHERE id = ?
            RETURNING *",
        )
        .bind(id)
        .fetch_one(conn)
        .await
        .change_context(JobQueryError)
        .attach("while trying to delete a background job")?;

        Ok(())
    }

    /// Finds a background job by its ID.
    pub async fn find_by_id(
        conn: &mut Connection,
        id: Uuid,
    ) -> Result<Self, Report<JobQueryError>> {
        sqlx::query_as::<_, BackgroundJob>("SELECT * FROM background_jobs WHERE id = ?")
            .bind(id)
            .fetch_one(conn)
            .await
            .change_context(JobQueryError)
            .attach("while trying to find background job metadata by id")
    }

    /// Retrieves and marks the next pending job as running.
    ///
    /// Jobs are selected based on:
    /// 1. Status must be `Enqueued`
    /// 2. Either never retried, or retry timeout has passed (exponential backoff)
    /// 3. Ordered by priority (descending) then creation time (ascending)
    pub async fn pull_next_pending(
        conn: &mut Connection,
        now: Option<Timestamp>,
    ) -> Result<Option<Self>, Report<JobQueryError>> {
        // SQLite's default bundle library does not come with power function,
        // manual implementation is needed using bit shift (2 << n == 2^(n+1))
        //
        // This operation is a bit heavy!
        sqlx::query_as::<_, BackgroundJob>(
            r#"
            UPDATE background_jobs
            SET last_retry = CURRENT_TIMESTAMP,
                retries = retries + 1,
                status = 'running'
            WHERE id IN (
                SELECT id FROM background_jobs
                WHERE status = 'enqueued'
                   AND (last_retry IS NULL
                   OR datetime(?) >= datetime(last_retry, '+' ||
                      CASE WHEN retries <= 0 THEN 0
                      ELSE 2 << (retries - 1)
                      END || ' minutes'))
                ORDER BY priority DESC, created_at ASC
                LIMIT 1
            )
            RETURNING *"#,
        )
        .bind(now.unwrap_or_else(Timestamp::now))
        .fetch_optional(conn)
        .await
        .change_context(JobQueryError)
        .attach("while trying to find the next pending background job")
    }

    /// Requeues a job or marks it as failed based on retry count.
    ///
    /// If `max_retries` is specified and the job has exceeded that limit,
    /// the job will be marked as `Failed`. Otherwise, it will be requeued
    /// as `Enqueued`.
    ///
    /// It returns the new status of the job after the operation.
    pub async fn requeue_or_fail(
        conn: &mut Connection,
        id: Uuid,
        max_retries: Option<u16>,
    ) -> Result<JobStatus, Report<JobQueryError>> {
        sqlx::query_scalar::<_, JobStatus>(
            r#"
            UPDATE background_jobs
            SET status = CASE
                WHEN ? IS NOT NULL AND retries + 1 > ? THEN 'failed'
                ELSE 'enqueued'
            END
            WHERE id = ?
            RETURNING status"#,
        )
        .bind(max_retries)
        .bind(max_retries)
        .bind(id)
        .fetch_one(conn)
        .await
        .change_context(JobQueryError)
        .attach("while trying to requeue a background job")
    }
}

/// Builder for creating a new background job.
///
/// This struct is used to construct a new job with various optional parameters
/// before enqueueing it to the database.
#[derive(Builder)]
#[must_use = "this does not do anything unless it is called to execute"]
pub struct NewBackgroundJob {
    #[builder(default = Uuid::new_v4())]
    pub id: Uuid,
    pub kind: &'static str,
    pub created_at: Option<Timestamp>,

    #[builder(setters(name = "data_internal", vis = ""))]
    pub data: String,

    #[builder(default = 0)]
    pub priority: i16,
}

type DataSetBuilder<S> = NewBackgroundJobBuilder<new_background_job_builder::SetData<S>>;

impl<S> NewBackgroundJobBuilder<S>
where
    S: new_background_job_builder::State,
{
    /// Sets the job data by serializing it to JSON.
    pub fn data<D>(self, data: D) -> Result<DataSetBuilder<S>, serde_json::Error>
    where
        D: serde::Serialize,
        S::Data: new_background_job_builder::IsUnset,
    {
        let data = serde_json::to_string(&data)?;
        Ok(self.data_internal(data))
    }
}

impl NewBackgroundJob {
    /// Enqueues the job to the database.
    ///
    /// It returns the UUID of the enqueued job.
    pub async fn enqueue(self, conn: &mut Connection) -> Result<Uuid, Report<JobQueryError>> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO background_jobs(id, created_at, type, data, priority)
            VALUES (?, ?, ?, ?, ?)
            RETURNING id"#,
        )
        .bind(self.id)
        .bind(self.created_at.unwrap_or_else(Timestamp::now))
        .bind(self.kind)
        .bind(self.data)
        .bind(self.priority)
        .fetch_one(conn)
        .await
        .change_context(JobQueryError)
        .attach("while trying to enqueue a background job to the database")
    }

    /// Enqueues this job only if no other job of the same type exists.
    ///
    /// This method ensures that only one job of a given type can be enqueued
    /// at a time. If a job of the same type exists and has failed, it will be
    /// deleted first before attempting to enqueue.
    pub async fn enqueue_unique(
        self,
        conn: &mut Connection,
    ) -> Result<Option<Uuid>, Report<JobQueryError>> {
        // Delete the existing job of the same type if it failed previously.
        sqlx::query(
            r#"
            DELETE FROM background_jobs
            WHERE type = ? AND status = 'failed'"#,
        )
        .bind(self.kind)
        .execute(&mut *conn)
        .await
        .change_context(JobQueryError)
        .attach("while trying to enqueue a background job to the database")?;

        let query = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO background_jobs (
                id, created_at, type,
                data, priority
            )
                SELECT ?, ?, ?, ?, ?
                WHERE NOT EXISTS (
                    SELECT * FROM background_jobs
                    WHERE type = ?
                )
            RETURNING id"#,
        );

        let query = query
            .bind(self.id)
            .bind(self.created_at.unwrap_or_else(Timestamp::now))
            .bind(self.kind)
            .bind(self.data)
            .bind(self.priority)
            .bind(self.kind);

        query
            .fetch_optional(conn)
            .await
            .change_context(JobQueryError)
            .attach("while trying to enqueue a background job to the database")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::str::FromStr;

    #[tokio::test]
    async fn enqueue_should_create_job_with_specified_id() {
        let pool = crate::testing::setup().await;
        let mut conn = pool.begin().await.unwrap();

        let id = Uuid::from_str("8d7b519e-6b0e-40de-98f7-c85f7792f7fc").unwrap();
        let builder = NewBackgroundJob::builder()
            .id(id)
            .kind("test")
            .priority(100)
            .data(json!({ "world": "hello" }))
            .expect("data should be serializable");

        let result = builder.build().enqueue(&mut conn).await.unwrap();
        assert_eq!(result, id);
    }

    #[tokio::test]
    async fn enqueue_unique_should_prevent_duplicate_jobs() {
        let pool = crate::testing::setup().await;
        let mut conn = pool.begin().await.unwrap();

        let id = Uuid::from_str("8d7b519e-6b0e-40de-98f7-c85f7792f7fc").unwrap();
        let current_job_id = enqueue_job()
            .conn(&mut conn)
            .id(id)
            .kind("test")
            .priority(100)
            .data(json!({ "hello": "world" }))
            .call()
            .await;

        assert_eq!(current_job_id, id);

        // Attempt to enqueue another job of the same type
        let builder = NewBackgroundJob::builder()
            .kind("test")
            .priority(12)
            .data(json!({ "world": "hello" }))
            .expect("data should be serializable");

        let query = builder.build().enqueue_unique(&mut conn).await.unwrap();
        assert!(query.is_none(), "should not enqueue duplicate job");
    }

    #[tokio::test]
    async fn find_by_id_should_return_job_metadata() {
        let pool = crate::testing::setup().await;
        let mut conn = pool.begin().await.unwrap();

        let job_id = enqueue_job()
            .conn(&mut conn)
            .kind("task1")
            .data(json!({}))
            .call()
            .await;

        let metadata = BackgroundJob::find_by_id(&mut conn, job_id).await.unwrap();
        assert_eq!(metadata.id, job_id);
        assert_eq!(metadata.data, "{}");
        assert_eq!(metadata.last_retry, None);
        assert_eq!(metadata.retries, 0);
        assert_eq!(metadata.status, JobStatus::Enqueued);
    }

    #[tokio::test]
    async fn pull_next_pending_should_respect_priority_order() {
        let pool = crate::testing::setup().await;
        let mut conn = pool.begin().await.unwrap();

        let [task_one, task_two, task_three, task_four] =
            prepare_jobs_for_queueing(&mut conn).await;

        // Expected order: highest priority first, then oldest
        // task_two (priority 100), task_one (priority 10, earlier),
        // task_three (priority 10, later), task_four (priority 1)
        let expected_sequence = [
            Some(task_two),
            Some(task_one),
            Some(task_three),
            Some(task_four),
            None,
        ];

        for expected in expected_sequence {
            assert_next_job(&mut conn, None, expected).await;
        }
    }

    #[tokio::test]
    async fn pull_next_pending_should_respect_retry_timeout() {
        let pool = crate::testing::setup().await;
        let mut conn = pool.begin().await.unwrap();

        let [task_one, task_two, task_three, task_four] =
            prepare_jobs_for_queueing(&mut conn).await;

        // Mark some jobs as already attempted
        sqlx::query(
            "UPDATE background_jobs
            SET last_retry = ?,
                retries = retries + 1
            WHERE id = ? OR id = ?",
        )
        .bind(Timestamp::from_str("2024-01-01T00:00:00Z").unwrap())
        .bind(task_three)
        .bind(task_four)
        .execute(&mut *conn)
        .await
        .unwrap();

        // Before timeout - should only get jobs that haven't been retried
        let ts = Timestamp::from_str("2023-12-25T00:00:00Z").unwrap();
        assert_next_job(&mut conn, Some(ts), Some(task_two)).await;
        assert_next_job(&mut conn, Some(ts), Some(task_one)).await;
        assert_next_job(&mut conn, Some(ts), None).await;

        // After timeout - should get previously retried jobs
        let ts = Timestamp::from_str("2024-01-01T00:05:00Z").unwrap();
        assert_next_job(&mut conn, Some(ts), Some(task_three)).await;
        assert_next_job(&mut conn, Some(ts), Some(task_four)).await;
        assert_next_job(&mut conn, Some(ts), None).await;
    }

    #[tokio::test]
    async fn requeue_or_fail_should_requeue_when_max_retries_not_set() {
        let pool = crate::testing::setup().await;
        let mut conn = pool.begin().await.unwrap();

        let job = enqueue_job()
            .conn(&mut conn)
            .kind("job1")
            .data(json!({}))
            .call()
            .await;

        BackgroundJob::requeue_or_fail(&mut conn, job, None)
            .await
            .unwrap();

        let job = BackgroundJob::find_by_id(&mut conn, job).await.unwrap();
        assert_eq!(job.status, JobStatus::Enqueued);
    }

    #[tokio::test]
    async fn requeue_or_fail_should_mark_as_failed_when_max_retries_exceeded() {
        let pool = crate::testing::setup().await;
        let mut conn = pool.begin().await.unwrap();

        let job = enqueue_job()
            .conn(&mut conn)
            .kind("job1")
            .data(json!({}))
            .call()
            .await;

        // Initially requeue
        BackgroundJob::requeue_or_fail(&mut conn, job, None)
            .await
            .unwrap();

        let metadata = BackgroundJob::find_by_id(&mut conn, job).await.unwrap();
        assert_eq!(metadata.status, JobStatus::Enqueued);

        // Simulate multiple retries
        sqlx::query("UPDATE background_jobs SET retries = 3 WHERE id = ?")
            .bind(job)
            .execute(&mut *conn)
            .await
            .unwrap();

        // Should fail when max_retries is exceeded
        BackgroundJob::requeue_or_fail(&mut conn, job, Some(1))
            .await
            .unwrap();

        let metadata = BackgroundJob::find_by_id(&mut conn, job).await.unwrap();
        assert_eq!(metadata.status, JobStatus::Failed);
    }

    /// Helper function to enqueue a job with customizable parameters.
    #[bon::builder]
    async fn enqueue_job(
        conn: &mut Connection,
        id: Option<Uuid>,
        created_at: Option<Timestamp>,
        kind: &'static str,
        priority: Option<i16>,
        data: serde_json::Value,
    ) -> Uuid {
        let created_at = created_at.unwrap_or_else(Timestamp::now);
        let builder = NewBackgroundJob::builder()
            .maybe_id(id)
            .created_at(created_at)
            .kind(kind)
            .maybe_priority(priority)
            .data(data)
            .expect("data should be serializable");

        builder.build().enqueue(conn).await.unwrap()
    }

    /// Asserts that the next job matches the expected ID.
    async fn assert_next_job(conn: &mut Connection, ts: Option<Timestamp>, expected: Option<Uuid>) {
        let actual = BackgroundJob::pull_next_pending(conn, ts)
            .await
            .unwrap()
            .map(|v| v.id);

        assert_eq!(actual, expected, "next job is not ordered as expected");
    }

    /// Prepares a set of jobs for queueing tests with different priorities and timestamps.
    async fn prepare_jobs_for_queueing(conn: &mut Connection) -> [Uuid; 4] {
        #[rustfmt::skip]
        let jobs = [
            ("2024-01-01T00:00:00Z", "a6b4fa28-40e7-4a07-b03d-2e3173016865", "job1", 10),
            ("2024-01-01T00:00:00Z", "03ced58f-7792-4ac1-b9bd-b0e97c906948", "job2", 100),
            ("2024-01-01T01:00:00Z", "43ca3d78-d3fd-4a30-95d3-d0c1d50f27f0", "job3", 10),
            ("2024-01-01T01:30:00Z", "c7fa0962-73b9-4c25-bbbd-b4bea4f14e3f", "job4", 1),
        ];

        let mut ids = [Uuid::nil(); 4];
        for (i, (created_at, id, kind, priority)) in jobs.into_iter().enumerate() {
            let id = NewBackgroundJob::builder()
                .id(Uuid::from_str(id).unwrap())
                .created_at(Timestamp::from_str(created_at).unwrap())
                .kind(kind)
                .priority(priority)
                .data(json!({}))
                .unwrap()
                .build()
                .enqueue(conn)
                .await
                .unwrap();

            ids[i] = id;
        }

        ids
    }
}
