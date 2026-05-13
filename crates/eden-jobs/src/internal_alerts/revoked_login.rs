use eden_background_worker::BackgroundJob;
use eden_common::domain::notifier::LinkedMcAccountLogin;
use erased_report::ErasedReport;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

use crate::JobContext;

#[derive(Debug, Deserialize, Serialize)]
pub struct AlertRevokedLoginJob(pub LinkedMcAccountLogin);

impl BackgroundJob for AlertRevokedLoginJob {
    const TYPE: &'static str = "eden::notification::revoked_login";
    const TIMEOUT: Duration = Duration::from_secs(30);

    type Context = Arc<JobContext>;

    #[tracing::instrument(skip_all)]
    async fn run(&self, ctx: Self::Context) -> Result<(), ErasedReport> {
        ctx.notifier().revoked_login(&self.0).await
    }
}
