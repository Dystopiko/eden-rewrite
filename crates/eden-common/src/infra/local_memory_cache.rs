use async_trait::async_trait;
use dashmap::DashMap;
use eden_model::{
    common::normalize_ip_into_trust_cidr,
    tables::{
        linked_mc_account_view::LinkedMcAccountView,
        mc_account_link_challenge::McAccountLinkChallenge, member_cidr_trust::MemberCidrTrust,
        member_view::MemberView, settings::Settings, tokens::Token,
    },
};
use erased_report::ErasedReport;
use ipnet::IpNet;
use moka::future::Cache;
use std::{fmt, net::IpAddr, sync::Arc, time::Duration};
use twilight_model::id::{
    Id,
    marker::{GuildMarker, UserMarker},
};
use uuid::Uuid;

use crate::{domain, token::HashedToken};

/// An in-memory cache implementation backed by [`moka::future::Cache`].
pub struct LocalMemoryCache {
    linked_mc_accounts: Cache<Uuid, LinkedMcAccountView>,
    link_challenges_by_code: Cache<String, McAccountLinkChallenge>,
    link_challenges: Cache<Uuid, McAccountLinkChallenge>,
    member_cidr_trust_entries: Cache<Id<UserMarker>, Arc<DashMap<IpNet, MemberCidrTrust>>>,
    member_views: Cache<Id<UserMarker>, MemberView>,
    settings: Cache<Id<GuildMarker>, Settings>,
    tokens: Cache<String, Token>,
}

impl LocalMemoryCache {
    /// Creates a new [`LocalMemoryCacheBuilder`] with default TTL values.
    pub fn builder() -> LocalMemoryCacheBuilder {
        LocalMemoryCacheBuilder::new()
    }
}

#[async_trait]
impl domain::Cache for LocalMemoryCache {
    async fn invalidate_all(&self) -> Result<(), ErasedReport> {
        self.linked_mc_accounts.invalidate_all();
        self.link_challenges_by_code.invalidate_all();
        self.link_challenges.invalidate_all();
        self.member_cidr_trust_entries.invalidate_all();
        self.member_views.invalidate_all();
        self.settings.invalidate_all();
        self.tokens.invalidate_all();
        Ok(())
    }

    async fn find_member_cidr_trust_entry(
        &self,
        member_id: Id<UserMarker>,
        ip: IpAddr,
    ) -> Result<Option<MemberCidrTrust>, ErasedReport> {
        let cidr = normalize_ip_into_trust_cidr(ip);
        let result = self
            .member_cidr_trust_entries
            .get(&member_id)
            .await
            .and_then(|m| m.get(&cidr).map(|v| v.clone()));

        Ok(result)
    }

    async fn find_linked_mc_account(
        &self,
        uuid: Uuid,
    ) -> Result<Option<LinkedMcAccountView>, ErasedReport> {
        Ok(self.linked_mc_accounts.get(&uuid).await)
    }

    async fn find_link_challenge_by_code(
        &self,
        hashed_code: &str,
    ) -> Result<Option<McAccountLinkChallenge>, ErasedReport> {
        Ok(self.link_challenges_by_code.get(hashed_code).await)
    }

    async fn find_link_challenge_in_progress(
        &self,
        id: Uuid,
    ) -> Result<Option<McAccountLinkChallenge>, ErasedReport> {
        Ok(self.link_challenges.get(&id).await)
    }

    async fn find_member_view(
        &self,
        discord_user_id: Id<UserMarker>,
    ) -> Result<Option<MemberView>, ErasedReport> {
        Ok(self.member_views.get(&discord_user_id).await)
    }

    async fn find_settings(&self, id: Id<GuildMarker>) -> Result<Option<Settings>, ErasedReport> {
        Ok(self.settings.get(&id).await)
    }

    async fn find_token(&self, hashed_token: &HashedToken) -> Result<Option<Token>, ErasedReport> {
        Ok(self.tokens.get(&hashed_token.encode()).await)
    }

    async fn update_link_challenge(
        &self,
        entry: &McAccountLinkChallenge,
    ) -> Result<(), ErasedReport> {
        self.link_challenges.insert(entry.id, entry.clone()).await;
        self.link_challenges_by_code
            .insert(entry.hashed_code.clone(), entry.clone())
            .await;

        Ok(())
    }

    async fn update_mc_linked_account(
        &self,
        entry: &LinkedMcAccountView,
    ) -> Result<(), ErasedReport> {
        self.linked_mc_accounts
            .insert(entry.uuid, entry.clone())
            .await;

        Ok(())
    }

    async fn update_member_cidr_trust(&self, entry: &MemberCidrTrust) -> Result<(), ErasedReport> {
        self.member_cidr_trust_entries
            .entry(entry.member_id.cast())
            .or_default()
            .await
            .value()
            .insert(entry.cidr, entry.clone());

        Ok(())
    }

    async fn update_member_view(
        &self,
        discord_user_id: Id<UserMarker>,
        view: &MemberView,
    ) -> Result<(), ErasedReport> {
        self.member_views
            .insert(discord_user_id, view.clone())
            .await;

        Ok(())
    }

    async fn update_settings(&self, settings: &Settings) -> Result<(), ErasedReport> {
        self.settings
            .insert(settings.org_guild_id.cast(), settings.clone())
            .await;

        Ok(())
    }

    async fn update_token(
        &self,
        hashed_token: &HashedToken,
        metadata: &Token,
    ) -> Result<(), ErasedReport> {
        self.tokens
            .insert(hashed_token.encode(), metadata.clone())
            .await;
        Ok(())
    }
}

impl fmt::Debug for LocalMemoryCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalMemoryCache").finish_non_exhaustive()
    }
}

/// Builder for [`LocalMemoryCache`] that allows customizing the TTL of each
/// internal cache segment.
#[must_use = "builders do not do anything unless you build them"]
pub struct LocalMemoryCacheBuilder {
    linked_mc_account_ttl: Duration,
    link_challenge_ttl: Duration,
    member_cidr_trust_ttl: Duration,
    member_view_ttl: Duration,
    settings_ttl: Duration,
    token_ttl: Duration,
}

impl LocalMemoryCacheBuilder {
    /// Creates a new builder with default TTL values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the TTL for the linked Minecraft accounts cache.
    ///
    /// **Default:** 30 seconds.
    pub fn linked_mc_account_ttl(mut self, duration: Duration) -> Self {
        self.linked_mc_account_ttl = duration;
        self
    }

    /// Sets the TTL for the link challenge caches (by code and by ID).
    ///
    /// **Default:** 15 minutes.
    pub fn link_challenge_ttl(mut self, duration: Duration) -> Self {
        self.link_challenge_ttl = duration;
        self
    }

    /// Sets the TTL for the member CIDR trust entries cache.
    ///
    /// **Default:** 30 minutes.
    pub fn member_cidr_trust_ttl(mut self, duration: Duration) -> Self {
        self.member_cidr_trust_ttl = duration;
        self
    }

    /// Sets the TTL for the member views cache.
    ///
    /// **Default:** 30 minutes.
    pub fn member_view_ttl(mut self, duration: Duration) -> Self {
        self.member_view_ttl = duration;
        self
    }

    /// Sets the TTL for the guild settings cache.
    ///
    /// **Default:** 1 hour.
    pub fn settings_ttl(mut self, duration: Duration) -> Self {
        self.settings_ttl = duration;
        self
    }

    /// Sets the TTL for the token cache.
    ///
    /// **Default:** 5 seconds.
    pub fn token_ttl(mut self, duration: Duration) -> Self {
        self.token_ttl = duration;
        self
    }

    /// Consumes the builder and returns a configured [`LocalMemoryCache`].
    pub fn build(self) -> LocalMemoryCache {
        LocalMemoryCache {
            linked_mc_accounts: Cache::builder()
                .name("eden::local_memory_cache::linked_mc_accounts")
                .time_to_live(self.linked_mc_account_ttl)
                .build(),
            link_challenges_by_code: Cache::builder()
                .name("eden::local_memory_cache::link_challenges_by_code")
                .time_to_live(self.link_challenge_ttl)
                .build(),
            link_challenges: Cache::builder()
                .name("eden::local_memory_cache::link_challenges")
                .time_to_live(self.link_challenge_ttl)
                .build(),
            member_cidr_trust_entries: Cache::builder()
                .name("eden::local_memory_cache::member_cidr_trust_entries")
                .time_to_live(self.member_cidr_trust_ttl)
                .build(),
            member_views: Cache::builder()
                .name("eden::local_memory_cache::member_views")
                .time_to_live(self.member_view_ttl)
                .build(),
            settings: Cache::builder()
                .name("eden::local_memory_cache::settings")
                .time_to_live(self.settings_ttl)
                .build(),
            tokens: Cache::builder()
                .name("eden::local_memory_cache::tokens")
                .time_to_live(self.token_ttl)
                .build(),
        }
    }
}

impl Default for LocalMemoryCacheBuilder {
    fn default() -> Self {
        Self {
            // `POST /sessions` depend on these tables and attacks may happen so we need
            // to make it resilient as possible from (D)DoS'ing the database.
            member_cidr_trust_ttl: Duration::from_mins(30),
            member_view_ttl: Duration::from_mins(30),

            linked_mc_account_ttl: Duration::from_secs(30),
            link_challenge_ttl: Duration::from_mins(15),
            settings_ttl: Duration::from_hours(1),
            token_ttl: Duration::from_secs(5),
        }
    }
}
