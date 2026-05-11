use async_trait::async_trait;
use erased_report::ErasedReport;
use std::fmt;
use twilight_model::{
    guild::Member,
    id::Id,
    id::marker::{GuildMarker, UserMarker},
};

use crate::domain::notifier::LoginMetadata;

/// An abstract interface that allows for integration with Eden through
/// Discord with the help of Discord API.
#[mockall::automock]
#[async_trait]
pub trait DiscordClient: fmt::Debug + Send + Sync + 'static {
    async fn fetch_member(
        &self,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
    ) -> Result<GetMemberResult, ErasedReport>;

    async fn notify_pending_login(&self, metadata: &LoginMetadata) -> Result<(), ErasedReport>;
}

/// Result of fetching a member from [`DiscordService::fetch_member`].
pub enum GetMemberResult {
    BotMissing,
    MemberMissing,
    Found(Box<Member>),
}
