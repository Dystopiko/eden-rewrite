use eden_background_worker::BackgroundJob;
use eden_model::alerts::command::CommandAlert;
use erased_report::ErasedReport;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

use crate::JobContext;

#[derive(Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct AlertCommandJob(pub CommandAlert);

impl BackgroundJob for AlertCommandJob {
    const TYPE: &'static str = "eden::internal_alerts::command";
    const TIMEOUT: Duration = Duration::from_mins(1);

    type Context = Arc<JobContext>;

    #[tracing::instrument(skip_all, fields(
        alert.command = ?self.0.command,
        alert.executor = ?self.0.source
    ))]
    async fn run(&self, ctx: Self::Context) -> Result<(), ErasedReport> {
        ctx.notifier().admin_used_command(&self.0).await
    }
}
