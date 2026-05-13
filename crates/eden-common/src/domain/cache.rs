use async_trait::async_trait;
use eden_model::tables::{
    linked_mc_account_view::LinkedMcAccountView, mc_account_link_challenge::McAccountLinkChallenge,
    member_cidr_trust::MemberCidrTrust, member_view::MemberView, settings::Settings, tokens::Token,
};
use erased_report::ErasedReport;
use std::{fmt, net::IpAddr};
use twilight_model::id::{
    Id,
    marker::{GuildMarker, UserMarker},
};
use uuid::Uuid;

use crate::token::HashedToken;

/// This trait is the interface of every Eden cache system should implement
/// to access frequently used data for services particularly the Eden API server.
#[mockall::automock]
#[async_trait]
pub trait Cache: fmt::Debug + Send + Sync + 'static {
    async fn invalidate_all(&self) -> Result<(), ErasedReport>;

    // Finders //
    async fn find_member_cidr_trust_entry(
        &self,
        member_id: Id<UserMarker>,
        ip: IpAddr,
    ) -> Result<Option<MemberCidrTrust>, ErasedReport>;

    async fn find_linked_mc_account(
        &self,
        uuid: Uuid,
    ) -> Result<Option<LinkedMcAccountView>, ErasedReport>;

    async fn find_link_challenge_by_code(
        &self,
        hashed_code: &str,
    ) -> Result<Option<McAccountLinkChallenge>, ErasedReport>;

    async fn find_link_challenge_in_progress(
        &self,
        mc_uuid: Uuid,
    ) -> Result<Option<McAccountLinkChallenge>, ErasedReport>;

    async fn find_member_view(
        &self,
        discord_user_id: Id<UserMarker>,
    ) -> Result<Option<MemberView>, ErasedReport>;

    async fn find_settings(&self, id: Id<GuildMarker>) -> Result<Option<Settings>, ErasedReport>;

    async fn find_token(&self, hashed_token: &HashedToken) -> Result<Option<Token>, ErasedReport>;

    // Updaters //
    async fn update_link_challenge(
        &self,
        entry: &McAccountLinkChallenge,
    ) -> Result<(), ErasedReport>;

    async fn update_mc_linked_account(
        &self,
        entry: &LinkedMcAccountView,
    ) -> Result<(), ErasedReport>;

    async fn update_member_cidr_trust(&self, entry: &MemberCidrTrust) -> Result<(), ErasedReport>;

    async fn update_member_view(
        &self,
        discord_user_id: Id<UserMarker>,
        view: &MemberView,
    ) -> Result<(), ErasedReport>;

    async fn update_settings(&self, settings: &Settings) -> Result<(), ErasedReport>;

    async fn update_token(
        &self,
        hashed_token: &HashedToken,
        metadata: &Token,
    ) -> Result<(), ErasedReport>;
}
