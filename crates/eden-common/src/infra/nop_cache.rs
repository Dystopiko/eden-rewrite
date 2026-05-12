use async_trait::async_trait;
use eden_model::tables::{
    linked_mc_account_view::LinkedMcAccountView, mc_account_link_challenge::McAccountLinkChallenge,
    member_cidr_trust::MemberCidrTrust, member_view::MemberView, settings::Settings, tokens::Token,
};
use erased_report::ErasedReport;
use std::net::IpAddr;
use twilight_model::id::{
    Id,
    marker::{GuildMarker, UserMarker},
};
use uuid::Uuid;

use crate::{domain, token::HashedToken};

#[derive(Debug)]
pub struct NopCache;

#[async_trait]
impl domain::Cache for NopCache {
    async fn invalidate_all(&self) -> Result<(), ErasedReport> {
        Ok(())
    }

    async fn find_member_cidr_trust_entry(
        &self,
        _member_id: Id<UserMarker>,
        _ip: IpAddr,
    ) -> Result<Option<MemberCidrTrust>, ErasedReport> {
        Ok(None)
    }

    async fn find_linked_mc_account(
        &self,
        _uuid: Uuid,
    ) -> Result<Option<LinkedMcAccountView>, ErasedReport> {
        Ok(None)
    }

    async fn find_link_challenge_by_code(
        &self,
        _hashed_code: &str,
    ) -> Result<Option<McAccountLinkChallenge>, ErasedReport> {
        Ok(None)
    }

    async fn find_link_challenge_in_progress(
        &self,
        _id: Uuid,
    ) -> Result<Option<McAccountLinkChallenge>, ErasedReport> {
        Ok(None)
    }

    async fn find_settings(&self, _id: Id<GuildMarker>) -> Result<Option<Settings>, ErasedReport> {
        Ok(None)
    }

    async fn find_token(&self, _hashed_token: &HashedToken) -> Result<Option<Token>, ErasedReport> {
        Ok(None)
    }

    async fn update_link_challenge(
        &self,
        _entry: &McAccountLinkChallenge,
    ) -> Result<(), ErasedReport> {
        Ok(())
    }

    async fn update_mc_linked_account(
        &self,
        _entry: &LinkedMcAccountView,
    ) -> Result<(), ErasedReport> {
        Ok(())
    }

    async fn update_member_cidr_trust(&self, _entry: &MemberCidrTrust) -> Result<(), ErasedReport> {
        Ok(())
    }

    async fn update_member_view(
        &self,
        _discord_user_id: Id<UserMarker>,
        _view: &MemberView,
    ) -> Result<(), ErasedReport> {
        Ok(())
    }

    async fn update_settings(&self, _settings: &Settings) -> Result<(), ErasedReport> {
        Ok(())
    }

    async fn update_token(
        &self,
        _hashed_token: &HashedToken,
        _metadata: &Token,
    ) -> Result<(), ErasedReport> {
        Ok(())
    }
}
