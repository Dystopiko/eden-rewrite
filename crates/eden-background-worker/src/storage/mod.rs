use eden_model::tables::background_job::QueryError;
use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type, query::QueryAs};
use uuid::Uuid;

#[derive(Clone, Debug, FromRow)]
pub struct Row {
    pub id: Uuid,
    pub created_at: Timestamp,
    pub job_type: String,
    pub data: serde_json::Value,
    pub priority: i32,
    #[allow(unused)]
    pub status: JobStatus,
    pub last_retry: Option<Timestamp>,
    pub retries: i32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case", type_name = "background_job_status")]
pub enum JobStatus {
    Enqueued,
    Running,
    Failed,
}

/// Deletes a background job by ID.
pub async fn delete(
    conn: &mut eden_postgres::Connection,
    id: Uuid,
) -> Result<(), Report<QueryError>> {
    sqlx::query(
        "DELETE FROM background_jobs
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .fetch_one(conn)
    .await
    .change_context(QueryError)
    .attach("while trying to delete a background job")?;

    Ok(())
}

/// Pulls up to `limit` pending jobs, marking them as running.
pub async fn pull_next_pending(
    conn: &mut eden_postgres::Connection,
    now: Option<Timestamp>,
    limit: u32,
) -> Result<Vec<Row>, Report<QueryError>> {
    build_pull_next_pending_query(now, limit)
        .fetch_all(conn)
        .await
        .change_context(QueryError)
        .attach("while trying to find the next pending background jobs")
}

/// Either re-enqueues a failed job for retry, or marks it permanently failed
/// if it has exceeded `max_retries`. Returns the resulting [`JobStatus`].
pub async fn requeue_or_fail(
    conn: &mut eden_postgres::Connection,
    id: Uuid,
    max_retries: Option<u16>,
) -> Result<JobStatus, Report<QueryError>> {
    const QUERY: &str = r#"
        UPDATE background_jobs
        SET status = (
            CASE
                WHEN $1 IS NOT NULL AND retries + 1 > $1 THEN 'failed'
                ELSE 'enqueued'
            END
        )::background_job_status
        WHERE id = $2
        RETURNING status
    "#;

    sqlx::query_scalar::<_, JobStatus>(QUERY)
        .bind(max_retries.map(i32::from))
        .bind(id)
        .fetch_one(conn)
        .await
        .change_context(QueryError)
        .attach("while trying to requeue a background job")
}

/// Builds a query that atomically claims the next `limit` enqueued jobs,
/// applying exponential back-off based on each job's retry count.
fn build_pull_next_pending_query(
    now: Option<Timestamp>,
    limit: u32,
) -> QueryAs<'static, sqlx::Postgres, Row, sqlx::postgres::PgArguments> {
    const QUERY: &str = r#"
        WITH updated AS (
            UPDATE background_jobs
            SET
                last_retry = CURRENT_TIMESTAMP,
                retries    = retries + 1,
                status     = 'running'
            WHERE id IN (
                SELECT id
                FROM background_jobs
                WHERE status = 'enqueued'
                  AND (
                      -- First attempt: no previous retry recorded.
                      last_retry IS NULL
                      -- Subsequent attempts: enough time has elapsed since the
                      -- last retry, based on exponential back-off.
                      OR $1 >= last_retry + (
                          CASE
                              WHEN retries <= 0 THEN INTERVAL '0 minutes'
                              ELSE (2 ^ (retries - 1)) * INTERVAL '1 minute'
                          END
                      )
                  )
                ORDER BY priority DESC, created_at ASC
                LIMIT $2
            )
            RETURNING *
        )
        SELECT * FROM updated
        ORDER BY priority DESC, created_at ASC
    "#;

    sqlx::query_as::<_, Row>(QUERY)
        .bind(now.unwrap_or_else(Timestamp::now))
        .bind(i64::from(limit))
}
