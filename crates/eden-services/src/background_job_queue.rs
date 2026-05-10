use eden_background_worker::job::{BackgroundJob, EnqueueJobError};
use error_stack::{Report, ResultExt};
use std::fmt;
use uuid::Uuid;

use crate::DatabasePools;

#[must_use]
pub struct BackgroundJobQueue<'a> {
    pools: &'a DatabasePools,
}

impl<'a> BackgroundJobQueue<'a> {
    pub const fn new(pools: &'a DatabasePools) -> Self {
        Self { pools }
    }

    #[tracing::instrument(skip_all, fields(?job))]
    pub async fn enqueue_job<J: BackgroundJob + fmt::Debug>(
        &self,
        job: J,
    ) -> Result<Option<Uuid>, Report<EnqueueJobError>> {
        tracing::debug!("enqueuing job");

        let mut conn = self
            .pools
            .write()
            .await
            .change_context(EnqueueJobError::Database)?;

        let id = job.enqueue(&mut conn).await?;
        conn.commit()
            .await
            .change_context(EnqueueJobError::Database)?;

        Ok(id)
    }
}
