use eden_background_worker::BackgroundJob;
use eden_model::tables::mc_login_event::McLoginEvent;
use erased_report::ErasedReport;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

use crate::JobContext;

#[derive(Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct AlertGuestJoinedJob(pub McLoginEvent);

impl BackgroundJob for AlertGuestJoinedJob {
    const TYPE: &'static str = "eden::alerts::guest_joined";
    const TIMEOUT: Duration = Duration::from_secs(30);

    type Context = Arc<JobContext>;

    #[tracing::instrument(skip_all)]
    async fn run(&self, ctx: Self::Context) -> Result<(), ErasedReport> {
        ctx.app.notifier().guest_player_joined(&self.0).await
    }
}
