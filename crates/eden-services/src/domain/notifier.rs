use async_trait::async_trait;
use eden_minecraft_types::McEdition;
use eden_model::{alerts::command::CommandAlert, tables::mc_login_event::McLoginEvent};
use eden_timestamp::Timestamp;
use erased_report::ErasedReport;
use serde::{Deserialize, Serialize};
use std::{fmt, net::IpAddr};
use twilight_model::id::{Id, marker::UserMarker};
use uuid::Uuid;

/// A trait defining the interface for dispatching notifications across
/// user-configured communication platforms only available for authorized
/// organization members.
///
/// As of this release, implementations default to sending
/// notifications via Discord.
#[mockall::automock]
#[async_trait]
pub trait Notifier: fmt::Debug + Send + Sync + 'static {
    async fn admin_used_command(&self, metadata: &CommandAlert) -> Result<(), ErasedReport>;
    async fn guest_player_joined(&self, event: &McLoginEvent) -> Result<(), ErasedReport>;
    async fn revoked_login(&self, metadata: &LoginMetadata) -> Result<(), ErasedReport>;
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LoginMetadata {
    pub edition: McEdition,
    pub member_id: Id<UserMarker>,
    pub ip: IpAddr,
    pub issued_at: Timestamp,
    pub username: String,
    pub uuid: Uuid,
}
