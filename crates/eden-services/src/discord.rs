use async_trait::async_trait;
use eden_config::types::organization::Discord;
use eden_model::{
    alerts::command::CommandAlert,
    tables::{contributor::NewContributor, mc_login_event::McLoginEvent, member::NewMember},
};
use eden_timestamp::Timestamp;
use erased_report::ErasedReport;
use error_stack::{Report, ResultExt};
use std::fmt;
use thiserror::Error;
use twilight_model::{
    guild::Member,
    id::{Id, marker::UserMarker},
};

pub enum ResolveMemberResult {
    BotNotAddedInGuild,
    MemberNotAddedInGuild,
    Done(Box<Member>),
}

#[mockall::automock]
#[async_trait]
pub trait DiscordService: fmt::Debug + Send + Sync + 'static {
    async fn alert_guest_player_joined(&self, event: &McLoginEvent) -> Result<(), ErasedReport>;
    async fn log_command_by_admin(&self, alert: &CommandAlert) -> Result<(), ErasedReport>;

    async fn resolve_member_from_org_guild(
        &self,
        user_id: Id<UserMarker>,
    ) -> Result<ResolveMemberResult, ErasedReport>;
}

#[derive(Debug, Error)]
#[error("Failed to setup member")]
pub struct SetupMemberError;

pub async fn setup_member(
    conn: &mut eden_postgres::Transaction<'_>,
    config: &Discord,
    member: &Member,
) -> Result<(), Report<SetupMemberError>> {
    NewMember::builder()
        .discord_user_id(member.user.id)
        .maybe_joined_at(member.joined_at.map(Timestamp::from_twilight))
        .name(&member.user.name)
        .build()
        .upsert(conn)
        .await
        .change_context(SetupMemberError)?;

    if let Some(role_id) = config.ids.contributor
        && member.roles.contains(&role_id)
    {
        NewContributor::builder()
            .member_id(member.user.id)
            .build()
            .upsert(conn)
            .await
            .change_context(SetupMemberError)?;
    }

    Ok(())
}
