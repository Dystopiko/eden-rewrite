use async_trait::async_trait;
use dashmap::DashMap;
use eden_model::tables::{
    linked_mc_account_view::LinkedMcAccountView, mc_account_link_challenge::McAccountLinkChallenge,
    member_cidr_trust::MemberCidrTrust, settings::Settings,
};
use erased_report::ErasedReport;
use std::{collections::HashMap, fmt, net::IpAddr};
use tokio::sync::RwLock;
use twilight_model::id::{
    Id,
    marker::{GuildMarker, UserMarker},
};
use uuid::Uuid;

use crate::cache::Cache;

#[allow(clippy::type_complexity)]
pub struct EdenMemoryCache {
    linked_accounts_by_uuid: DashMap<Uuid, Option<LinkedMcAccountView>>,
    link_challenges_by_hashed_code: DashMap<String, Option<McAccountLinkChallenge>>,
    link_challenges_in_progress: DashMap<Uuid, Option<McAccountLinkChallenge>>,
    member_cidr_trust: RwLock<HashMap<(Id<UserMarker>, IpAddr), Option<MemberCidrTrust>>>,
    member_cidr_trust_entries: RwLock<HashMap<Uuid, MemberCidrTrust>>,
    settings: DashMap<Id<GuildMarker>, Option<Settings>>,
}

impl EdenMemoryCache {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            linked_accounts_by_uuid: DashMap::new(),
            link_challenges_by_hashed_code: DashMap::new(),
            link_challenges_in_progress: DashMap::new(),
            member_cidr_trust: RwLock::new(HashMap::new()),
            member_cidr_trust_entries: RwLock::new(HashMap::new()),
            settings: DashMap::new(),
        }
    }
}

// TODO: Make a dedicated thread for this cache here.
#[async_trait]
impl Cache for EdenMemoryCache {
    async fn clear(&self) -> Result<(), ErasedReport> {
        self.linked_accounts_by_uuid.clear();
        self.link_challenges_by_hashed_code.clear();
        self.link_challenges_in_progress.clear();
        self.member_cidr_trust.write().await.clear();
        self.settings.clear();
        Ok(())
    }

    async fn find_linked_account_view(
        &self,
        uuid: Uuid,
    ) -> Result<Option<LinkedMcAccountView>, ErasedReport> {
        let result = self
            .linked_accounts_by_uuid
            .entry(uuid)
            .or_default()
            .clone();

        Ok(result)
    }

    async fn find_link_challenge_by_code(
        &self,
        hashed_code: &str,
    ) -> Result<Option<McAccountLinkChallenge>, ErasedReport> {
        let result = self
            .link_challenges_by_hashed_code
            .entry(hashed_code.to_string())
            .or_default()
            .clone();

        Ok(result)
    }

    async fn find_link_challenge_in_progress(
        &self,
        id: Uuid,
    ) -> Result<Option<McAccountLinkChallenge>, erased_report::ErasedReport> {
        let result = self
            .link_challenges_in_progress
            .entry(id)
            .or_default()
            .clone();

        Ok(result)
    }

    async fn find_member_cidr_trust_entry(
        &self,
        member_id: Id<UserMarker>,
        ip: IpAddr,
    ) -> Result<Option<MemberCidrTrust>, ErasedReport> {
        let mut map = self.member_cidr_trust.write().await;
        let result = map.entry((member_id, ip)).or_default().clone();
        if result.is_some() {
            return Ok(result);
        }

        for (_, entry) in self.member_cidr_trust_entries.read().await.iter() {
            if entry.cidr.contains(&ip) {
                let key = (member_id, ip);
                map.insert(key, Some(entry.clone()));
                return Ok(Some(entry.clone()));
            }
        }

        Ok(None)
    }

    async fn find_settings(&self, id: Id<GuildMarker>) -> Result<Option<Settings>, ErasedReport> {
        self.settings.entry(id).or_insert(None);
        Ok(None)
    }

    async fn populate_member_cidr_trust_entries(
        &self,
        entries: &[MemberCidrTrust],
    ) -> Result<(), ErasedReport> {
        for entry in entries {
            let entry = entry.clone();
            self.member_cidr_trust_entries
                .write()
                .await
                .insert(entry.id, entry);
        }

        Ok(())
    }

    async fn update_link_challenge(
        &self,
        entry: &McAccountLinkChallenge,
    ) -> Result<(), ErasedReport> {
        self.link_challenges_by_hashed_code
            .entry(entry.hashed_code.to_string())
            .and_modify(|challenge| {
                *challenge = Some(entry.clone());
            })
            .or_insert_with(|| Some(entry.clone()));

        self.link_challenges_in_progress
            .entry(entry.id)
            .and_modify(|challenge| {
                *challenge = Some(entry.clone());
            })
            .or_insert_with(|| Some(entry.clone()));

        Ok(())
    }

    async fn update_linked_account_view(
        &self,
        entry: &LinkedMcAccountView,
    ) -> Result<(), ErasedReport> {
        self.linked_accounts_by_uuid
            .insert(entry.uuid, Some(entry.clone()));

        Ok(())
    }

    async fn update_member_cidr_trust_by_ip(
        &self,
        ip: IpAddr,
        entry: &MemberCidrTrust,
    ) -> Result<(), ErasedReport> {
        let key = (entry.member_id.cast(), ip);
        self.member_cidr_trust
            .write()
            .await
            .insert(key, Some(entry.clone()));

        Ok(())
    }

    async fn update_member_cidr_trust(&self, entry: &MemberCidrTrust) -> Result<(), ErasedReport> {
        self.member_cidr_trust_entries
            .write()
            .await
            .entry(entry.id)
            .and_modify(|v| *v = entry.clone())
            .or_insert_with(|| entry.clone());

        for (_, candidate) in self.member_cidr_trust.write().await.iter_mut() {
            let Some(candidate) = candidate else {
                continue;
            };

            if candidate.id != entry.id {
                continue;
            }
            *candidate = entry.clone();
        }

        Ok(())
    }

    async fn update_settings(&self, settings: &Settings) -> Result<(), ErasedReport> {
        self.settings
            .entry(settings.org_guild_id.cast())
            .and_modify(|old| *old = Some(settings.clone()))
            .or_insert_with(|| Some(settings.clone()));

        Ok(())
    }
}

impl Default for EdenMemoryCache {
    fn default() -> Self {
        Self::empty()
    }
}

impl fmt::Debug for EdenMemoryCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EdenMemoryCache").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use eden_minecraft_types::McEdition;
    use eden_model::tables::mc_account_link_challenge::{ChallengeStatus, McAccountLinkChallenge};
    use eden_timestamp::Timestamp;
    use std::net::{IpAddr, Ipv4Addr};
    use uuid::Uuid;

    use crate::cache::{Cache, EdenMemoryCache};

    #[tokio::test]
    async fn should_retrieve_in_progress_link_challenge_if_not_exists() {
        let cache = EdenMemoryCache::empty();

        let result = cache.find_link_challenge_by_code("code").await.unwrap();
        assert_eq!(result.as_ref(), None);

        let result = cache
            .find_link_challenge_in_progress(Uuid::nil())
            .await
            .unwrap();

        assert_eq!(result.as_ref(), None);
    }

    #[tokio::test]
    async fn should_retrieve_in_progress_link_challenge_if_exists() {
        let challenge_id = Uuid::new_v4();
        let expected: McAccountLinkChallenge = McAccountLinkChallenge {
            id: challenge_id,
            created_at: Timestamp::now(),
            hashed_code: "code".to_string(),
            expires_at: Timestamp::now(),
            player_uuid: Uuid::nil(),
            username: "john".to_string(),
            edition: McEdition::Java,
            ip_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            status: ChallengeStatus::InProgress,
            updated_at: None,
        };

        let cache = EdenMemoryCache::empty();
        cache.update_link_challenge(&expected).await.unwrap();

        let result = cache.find_link_challenge_by_code("code").await.unwrap();
        assert_eq!(result.as_ref(), Some(&expected));

        let result = cache
            .find_link_challenge_in_progress(challenge_id)
            .await
            .unwrap();

        assert_eq!(result.as_ref(), Some(&expected));
    }
}
