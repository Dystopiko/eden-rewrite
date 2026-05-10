use eden_background_worker::BackgroundJob;
use eden_model::tables::mc_login_event::NewMcLoginEvent;
use eden_services::background_job_queue::BackgroundJobQueue;
use erased_report::ErasedReport;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

use crate::{JobContext, alerts::guest_joined::AlertGuestJoinedJob};

#[derive(Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct OnPlayerJoinedJob(pub NewMcLoginEvent);

impl BackgroundJob for OnPlayerJoinedJob {
    const TYPE: &'static str = "eden::events::player_joined";
    const TIMEOUT: Duration = Duration::from_secs(30);

    type Context = Arc<JobContext>;

    #[tracing::instrument(skip_all)]
    async fn run(&self, ctx: Self::Context) -> Result<(), ErasedReport> {
        let event = &self.0;

        let mut conn = ctx.pools.write().await?;
        let event = event.insert(&mut conn).await?;
        conn.commit().await.map_err(ErasedReport::new)?;

        if event.member_id.is_none() {
            BackgroundJobQueue::new(&ctx.pools)
                .enqueue_job(AlertGuestJoinedJob(event.clone()))
                .await?;
        }

        Ok(())
    }
}
