use async_trait::async_trait;
use eden_model::tables::member_cidr_trust::MemberCidrTrust;
use eden_model::tables::settings::Settings;
use eden_model::tables::{
    linked_mc_account_view::LinkedMcAccountView, mc_account_link_challenge::McAccountLinkChallenge,
};
use erased_report::ErasedReport;
use std::fmt;
use std::net::IpAddr;
use twilight_model::id::Id;
use twilight_model::id::marker::{GuildMarker, UserMarker};
use uuid::Uuid;

pub mod memory;
pub mod nop;

pub use self::memory::EdenMemoryCache;
pub use self::nop::NopMemoryCache;

#[mockall::automock]
#[async_trait]
pub trait Cache: fmt::Debug + Send + Sync + 'static {
    async fn clear(&self) -> Result<(), ErasedReport>;

    async fn find_member_cidr_trust_entry(
        &self,
        member_id: Id<UserMarker>,
        ip: IpAddr,
    ) -> Result<Option<MemberCidrTrust>, ErasedReport>;

    async fn find_linked_account_view(
        &self,
        uuid: Uuid,
    ) -> Result<Option<LinkedMcAccountView>, ErasedReport>;

    async fn find_link_challenge_by_code(
        &self,
        hashed_code: &str,
    ) -> Result<Option<McAccountLinkChallenge>, ErasedReport>;

    async fn find_link_challenge_in_progress(
        &self,
        id: Uuid,
    ) -> Result<Option<McAccountLinkChallenge>, ErasedReport>;

    async fn find_settings(&self, id: Id<GuildMarker>) -> Result<Option<Settings>, ErasedReport>;

    async fn populate_member_cidr_trust_entries(
        &self,
        entries: &[MemberCidrTrust],
    ) -> Result<(), ErasedReport>;

    async fn update_link_challenge(
        &self,
        entry: &McAccountLinkChallenge,
    ) -> Result<(), ErasedReport>;

    async fn update_linked_account_view(
        &self,
        entry: &LinkedMcAccountView,
    ) -> Result<(), ErasedReport>;

    async fn update_member_cidr_trust(&self, entry: &MemberCidrTrust) -> Result<(), ErasedReport>;

    async fn update_member_cidr_trust_by_ip(
        &self,
        ip: IpAddr,
        entry: &MemberCidrTrust,
    ) -> Result<(), ErasedReport>;

    async fn update_settings(&self, settings: &Settings) -> Result<(), ErasedReport>;
}
