use eden_background_worker::BackgroundJob;
use eden_model::tables::member_cidr_trust::MemberCidrTrust;
use erased_report::ErasedReport;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

use crate::JobContext;

#[derive(Debug, Deserialize, Serialize)]
pub struct NotifyPendingIpLogin(pub MemberCidrTrust);

impl BackgroundJob for NotifyPendingIpLogin {
    const TYPE: &'static str = "eden::notification::pending_ip_login";
    const TIMEOUT: Duration = Duration::from_secs(30);

    type Context = Arc<JobContext>;

    #[tracing::instrument(skip_all)]
    async fn run(&self, _ctx: Self::Context) -> Result<(), ErasedReport> {
        Ok(())
    }
}
