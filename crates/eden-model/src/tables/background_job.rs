use bon::Builder;
use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use thiserror::Error;
use uuid::Uuid;

#[derive(Builder)]
#[must_use = "this does not do anything unless it is called to execute"]
pub struct NewBackgroundJob {
    #[builder(default = Uuid::new_v4())]
    pub id: Uuid,
    pub job_type: &'static str,
    pub created_at: Option<Timestamp>,
    #[builder(setters(name = "data_internal", vis = ""))]
    pub data: serde_json::Value,
    #[builder(default = 0)]
    pub priority: i16,
}

#[derive(Debug, Error)]
#[error("could not query background_jobs table")]
pub struct QueryError;

type DataSetBuilder<S> = NewBackgroundJobBuilder<new_background_job_builder::SetData<S>>;

impl<S> NewBackgroundJobBuilder<S>
where
    S: new_background_job_builder::State,
{
    pub fn data<D>(self, data: D) -> Result<DataSetBuilder<S>, serde_json::Error>
    where
        D: serde::Serialize,
        S::Data: new_background_job_builder::IsUnset,
    {
        let data = serde_json::to_value(&data)?;
        Ok(self.data_internal(data))
    }
}

impl NewBackgroundJob {
    pub async fn enqueue(
        self,
        conn: &mut eden_postgres::Connection,
    ) -> Result<Uuid, Report<QueryError>> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO background_jobs(id, created_at, job_type, data, priority)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id"#,
        )
        .bind(self.id)
        .bind(self.created_at.unwrap_or_else(Timestamp::now))
        .bind(self.job_type)
        .bind(self.data)
        .bind(self.priority)
        .fetch_one(conn)
        .await
        .change_context(QueryError)
        .attach("while trying to enqueue a background job to the database")
    }

    pub async fn enqueue_unique(
        self,
        conn: &mut eden_postgres::Transaction<'_>,
    ) -> Result<Option<Uuid>, Report<QueryError>> {
        // Delete the existing job of the same type if it failed previously.
        sqlx::query(
            r#"
            DELETE FROM background_jobs
            WHERE job_type = $1 AND status = 'failed'"#,
        )
        .bind(self.job_type)
        .execute(&mut **conn)
        .await
        .change_context(QueryError)
        .attach("while trying to enqueue a background job to the database")?;

        let query = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO background_jobs (id, created_at, job_type, data, priority)
                SELECT $1, $2, $3, $4, $5
                WHERE NOT EXISTS (SELECT * FROM background_jobs WHERE job_type = $3)
            RETURNING *"#,
        );

        let query = query
            .bind(self.id)
            .bind(self.created_at.unwrap_or_else(Timestamp::now))
            .bind(self.job_type)
            .bind(self.data)
            .bind(self.priority);

        query
            .fetch_optional(&mut **conn)
            .await
            .change_context(QueryError)
            .attach("while trying to enqueue a background job to the database")
    }
}
