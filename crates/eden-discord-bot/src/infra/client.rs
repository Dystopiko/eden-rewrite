use async_trait::async_trait;
use eden_common::domain::{
    DiscordClient, discord::GetMemberResult, notifier::LinkedMcAccountLogin,
};
use eden_twilight::http::{HttpFailReason, ResponseFutureExt};
use erased_report::{EraseReportExt, ErasedReport};
use std::sync::Arc;
use twilight_model::id::{
    Id,
    marker::{GuildMarker, UserMarker},
};

use crate::context::BotContext;

#[derive(Debug)]
pub struct DiscordClientImpl {
    http: Arc<twilight_http::Client>,
}

impl DiscordClientImpl {
    #[must_use]
    pub fn new(ctx: &BotContext) -> Arc<Self> {
        Arc::new(Self {
            http: ctx.http.clone(),
        })
    }
}

// Retrieved from: https://discord.com/developers/docs/topics/opcodes-and-status-codes#http
const UNKNOWN_GUILD: u64 = 10004;
const UNKNOWN_MEMBER: u64 = 10007;

#[async_trait]
impl DiscordClient for DiscordClientImpl {
    async fn fetch_member(
        &self,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
    ) -> Result<GetMemberResult, ErasedReport> {
        let result = self.http.guild_member(guild_id, user_id).model().await;
        let (reason, report) = match result {
            Ok(member) => return Ok(GetMemberResult::Found(Box::new(member))),
            Err(error) => (error.current_context().reason(), error),
        };

        match reason {
            HttpFailReason::Response(UNKNOWN_GUILD) => Ok(GetMemberResult::BotMissing),
            HttpFailReason::Response(UNKNOWN_MEMBER) => Ok(GetMemberResult::MemberMissing),
            _ => Err(report).erase_report(),
        }
    }

    async fn notify_pending_login(
        &self,
        _metadata: &LinkedMcAccountLogin,
    ) -> Result<(), ErasedReport> {
        Ok(())
    }
}
