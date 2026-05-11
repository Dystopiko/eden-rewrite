use eden_background_worker::BackgroundJob;
use eden_services::domain::notifier::LoginMetadata;
use erased_report::ErasedReport;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

use crate::JobContext;

#[derive(Debug, Deserialize, Serialize)]
pub struct NotifyPendingLoginJob(pub LoginMetadata);

impl BackgroundJob for NotifyPendingLoginJob {
    const TYPE: &'static str = "eden::notification::pending_login";
    const TIMEOUT: Duration = Duration::from_secs(30);

    type Context = Arc<JobContext>;

    #[tracing::instrument(skip_all)]
    async fn run(&self, ctx: Self::Context) -> Result<(), ErasedReport> {
        ctx.app.discord().notify_pending_login(&self.0).await
    }
}
