use eden_config::{Config, types::setup::InitialSettings};
use eden_model::tables::{
    linked_mc_account_view::LinkedMcAccountView,
    mc_account_link_challenge::McAccountLinkChallenge,
    member_cidr_trust::{MemberCidrTrust, NewMemberCidrTrust},
    settings::{NewSettings, Settings},
    tokens::Token,
};
use eden_postgres::error::QueryResultExt;
use erased_report::{EraseReportExt, ErasedReport, IntoErasedReportExt};
use error_stack::{Report, ResultExt};
use std::net::IpAddr;
use thiserror::Error;
use twilight_model::id::{Id, marker::UserMarker};
use uuid::Uuid;

use crate::{DatabasePools, domain::Cache, token::HashedToken};

/// Repository that caches database reads to a [cache provider].
///
/// [cache provider]: crate::domain::Cache
#[derive(Clone, Debug)]
#[must_use]
pub struct CachedRepository<'a> {
    cache: &'a dyn Cache,
    pools: &'a DatabasePools,
}

impl<'a> CachedRepository<'a> {
    pub fn new(cache: &'a dyn Cache, pools: &'a DatabasePools) -> Self {
        Self { cache, pools }
    }

    pub async fn find_link_challenge_by_code(
        &self,
        hashed_code: &str,
    ) -> Result<McAccountLinkChallenge, ErasedReport> {
        if let Some(cached) = self.cache.find_link_challenge_by_code(hashed_code).await? {
            return Ok(cached);
        }

        let mut conn = self.pools.read_prefer_primary().await?;
        let challenge = McAccountLinkChallenge::find_by_hashed_code(&mut conn, hashed_code)
            .await
            .erase_report()?;

        self.cache.update_link_challenge(&challenge).await?;
        Ok(challenge)
    }

    pub async fn find_link_challenge_in_progress(
        &self,
        uuid: Uuid,
    ) -> Result<McAccountLinkChallenge, ErasedReport> {
        if let Some(cached) = self.cache.find_link_challenge_in_progress(uuid).await? {
            return Ok(cached);
        }

        let mut conn = self.pools.read_prefer_primary().await?;
        let challenge = McAccountLinkChallenge::find_in_progress(&mut conn, uuid)
            .await
            .erase_report()?;

        self.cache.update_link_challenge(&challenge).await?;
        Ok(challenge)
    }

    pub async fn find_linked_mc_account_view(
        &self,
        mc_uuid: Uuid,
    ) -> Result<LinkedMcAccountView, ErasedReport> {
        if let Some(cached) = self.cache.find_linked_mc_account(mc_uuid).await? {
            return Ok(cached);
        }

        let mut conn = self.pools.read_prefer_primary().await?;
        let account = LinkedMcAccountView::from_mc_uuid(mc_uuid, &mut conn)
            .await
            .erase_report()?;

        self.cache.update_mc_linked_account(&account).await?;
        Ok(account)
    }

    pub async fn find_token(&self, hashed_token: &HashedToken) -> Result<Token, ErasedReport> {
        if let Some(cached) = self.cache.find_token(hashed_token).await? {
            return Ok(cached);
        }

        let mut conn = self.pools.read_prefer_primary().await?;
        let token = Token::find_by_hashed(&mut conn, hashed_token.as_bytes())
            .await
            .erase_report()?;

        self.cache.update_token(hashed_token, &token).await?;
        Ok(token)
    }

    pub async fn resolve_member_cidr_trust(
        &self,
        member_id: Id<UserMarker>,
        ip: IpAddr,
    ) -> Result<ResolvedCidrTrust, ErasedReport> {
        let cached = self
            .cache
            .find_member_cidr_trust_entry(member_id, ip)
            .await?;

        if let Some(cached) = cached {
            return Ok(ResolvedCidrTrust {
                created: false,
                value: cached,
            });
        }

        let mut conn = self.pools.write().await?;
        let mut entry = MemberCidrTrust::find(&mut conn, member_id, ip)
            .await
            .optional()?;

        let should_insert = entry.is_none();
        if should_insert {
            let result = NewMemberCidrTrust::builder()
                .cidr_from_ip(ip)
                .member_id(member_id)
                .build()
                .insert(&mut conn)
                .await?;

            entry = Some(result);
        }

        let entry = entry.expect("it has already unresolved both cases");

        self.cache.update_member_cidr_trust(&entry).await?;
        conn.commit().await.erase_report()?;

        Ok(ResolvedCidrTrust {
            created: should_insert,
            value: entry,
        })
    }

    pub async fn settings(&self, config: &Config) -> Result<Settings, Report<QuerySettingsError>> {
        // Make sure the organization's Discord guild ID is present
        let Some(org_guild_id) = config.organization.discord.as_ref().map(|v| v.guild_id) else {
            return Err(Report::new(QuerySettingsError::Missing));
        };

        let cached = self
            .cache
            .find_settings(org_guild_id)
            .await
            .map_err(|e| e.change_context(QuerySettingsError::General))?;

        if let Some(cached) = cached {
            return Ok(cached);
        }

        let mut conn = self
            .pools
            .write()
            .await
            .change_context(QuerySettingsError::General)?;

        // Use the existing settings or we can upsert them if needed.
        let settings = if let Some(settings) = Settings::find(org_guild_id, &mut conn)
            .await
            .change_context(QuerySettingsError::General)
            .optional()?
        {
            settings
        } else {
            NewSettings::builder()
                .org_guild_id(org_guild_id)
                .use_initial_settings(&config.setup.settings)
                .build()
                .upsert(&mut conn)
                .await
                .change_context(QuerySettingsError::General)?
        };

        conn.commit()
            .await
            .change_context(QuerySettingsError::General)?;

        self.cache
            .update_settings(&settings)
            .await
            .map_err(|e| e.change_context(QuerySettingsError::General))?;

        Ok(settings)
    }

    pub async fn update_settings(
        &self,
        config: &Config,
        settings: &InitialSettings,
    ) -> Result<Settings, Report<QuerySettingsError>> {
        // Make sure the organization's Discord guild ID is present
        let Some(org_guild_id) = config.organization.discord.as_ref().map(|v| v.guild_id) else {
            return Err(Report::new(QuerySettingsError::Missing));
        };

        let mut conn = self
            .pools
            .write()
            .await
            .change_context(QuerySettingsError::General)?;

        let settings = NewSettings::builder()
            .org_guild_id(org_guild_id)
            .use_initial_settings(settings)
            .build()
            .upsert(&mut conn)
            .await
            .change_context(QuerySettingsError::General)?;

        conn.commit()
            .await
            .change_context(QuerySettingsError::General)?;

        self.cache
            .update_settings(&settings)
            .await
            .map_err(|e| e.change_context(QuerySettingsError::General))?;

        Ok(settings)
    }
}

#[derive(Debug)]
pub struct ResolvedCidrTrust {
    pub created: bool,
    pub value: MemberCidrTrust,
}

/// Errors that can occur when querying organization's settings.
#[derive(Debug, Error)]
pub enum QuerySettingsError {
    #[error(
        "Failed to query Eden settings: `organization.discord.guild_id` is not \
        configured. Set `organization.discord.guild_id` for Eden to function \
        properly."
    )]
    Missing,

    #[error("Failed to query Eden settings")]
    General,
}
