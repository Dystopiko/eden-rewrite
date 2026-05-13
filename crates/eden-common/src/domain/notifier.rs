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
    async fn revoked_login(&self, metadata: &LinkedMcAccountLogin) -> Result<(), ErasedReport>;
}

/// This struct is similiar to [`McLoginEvent`] but it has [`member_id`] field non-nullable.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LinkedMcAccountLogin {
    pub created_at: Timestamp,
    pub member_id: Id<UserMarker>,
    pub ip: IpAddr,
    pub edition: McEdition,
    pub username: String,
    pub uuid: Uuid,
}

impl LinkedMcAccountLogin {
    #[must_use]
    pub fn from_table(row: McLoginEvent) -> Option<Self> {
        row.member_id
            .zip(row.username)
            .map(|(member_id, username)| Self {
                created_at: row.created_at,
                member_id: member_id.cast(),
                ip: row.ip_address,
                edition: row.edition,
                username,
                uuid: row.player_uuid,
            })
    }
}
