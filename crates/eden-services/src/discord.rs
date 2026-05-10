use async_trait::async_trait;
use eden_model::{alerts::command::CommandAlert, tables::mc_login_event::McLoginEvent};
use erased_report::ErasedReport;
use std::fmt;

#[mockall::automock]
#[async_trait]
pub trait DiscordService: fmt::Debug + Send + Sync + 'static {
    async fn alert_guest_player_joined(&self, event: &McLoginEvent) -> Result<(), ErasedReport>;
    async fn log_command_by_admin(&self, alert: &CommandAlert) -> Result<(), ErasedReport>;
}
