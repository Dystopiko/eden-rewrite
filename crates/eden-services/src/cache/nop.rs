use async_trait::async_trait;
use eden_model::tables::{
    linked_mc_account_view::LinkedMcAccountView, mc_account_link_challenge::McAccountLinkChallenge,
    member_cidr_trust::MemberCidrTrust, settings::Settings,
};
use erased_report::ErasedReport;
use std::net::IpAddr;
use twilight_model::id::{
    Id,
    marker::{GuildMarker, UserMarker},
};
use uuid::Uuid;

use crate::cache::Cache;

#[derive(Debug)]
pub struct NopMemoryCache;

#[async_trait]
impl Cache for NopMemoryCache {
    async fn clear(&self) -> Result<(), ErasedReport> {
        Ok(())
    }

    async fn find_linked_account_view(
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

    async fn find_member_cidr_trust_entry(
        &self,
        _member_id: Id<UserMarker>,
        _ip: IpAddr,
    ) -> Result<Option<MemberCidrTrust>, ErasedReport> {
        Ok(None)
    }

    async fn find_settings(&self, _id: Id<GuildMarker>) -> Result<Option<Settings>, ErasedReport> {
        Ok(None)
    }

    async fn populate_member_cidr_trust_entries(
        &self,
        _entries: &[MemberCidrTrust],
    ) -> Result<(), ErasedReport> {
        Ok(())
    }

    async fn update_link_challenge(
        &self,
        _entry: &McAccountLinkChallenge,
    ) -> Result<(), ErasedReport> {
        Ok(())
    }

    async fn update_linked_account_view(
        &self,
        _entry: &LinkedMcAccountView,
    ) -> Result<(), ErasedReport> {
        Ok(())
    }

    async fn update_member_cidr_trust_by_ip(
        &self,
        _ip: IpAddr,
        _entry: &MemberCidrTrust,
    ) -> Result<(), ErasedReport> {
        Ok(())
    }

    async fn update_member_cidr_trust(&self, _entry: &MemberCidrTrust) -> Result<(), ErasedReport> {
        Ok(())
    }

    async fn update_settings(&self, _settings: &Settings) -> Result<(), ErasedReport> {
        Ok(())
    }
}
