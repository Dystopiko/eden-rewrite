use eden_background_worker::BackgroundJob;
use eden_model::tables::mc_login_event::NewMcLoginEvent;
use erased_report::ErasedReport;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

use crate::JobContext;

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

        let mut conn = ctx.app.pools().write().await?;
        event.insert(&mut conn).await?;
        conn.commit().await.map_err(ErasedReport::new)?;

        Ok(())
    }
}
