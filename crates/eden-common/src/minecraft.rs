use eden_config::{Config, types::organization::minecraft::PerkId};
use eden_model::tables::member_view::MemberFlags;
use std::{collections::HashSet, sync::Arc};
use twilight_model::id::{Id, marker::UserMarker};
use uuid::Uuid;

#[derive(Clone, Debug)]
#[must_use]
pub struct McService {
    config: Arc<Config>,
}

impl McService {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn get_head_icon_url(&self, source: HeadIconSource<'_>) -> String {
        const HEAD_ICON_BASE_URL: &str = "https://minotar.net/avatar/";

        let mut url = HEAD_ICON_BASE_URL.to_string();
        match source {
            HeadIconSource::Username(username) => {
                let encoded_username = percent_encoding::percent_encode(
                    username.as_bytes(),
                    percent_encoding::NON_ALPHANUMERIC,
                );
                url.extend(encoded_username)
            }
            HeadIconSource::Uuid(uuid) => {
                // Minotar recommends removing the UUID dash strips
                url.push_str(&uuid.simple().to_string());
            }
        }

        url
    }

    #[must_use]
    pub fn resolve_perks(
        &self,
        member_flags: MemberFlags,
        member_id: Option<Id<UserMarker>>,
        mc_uuid: Option<Uuid>,
    ) -> Vec<String> {
        let perks = &self.config.organization.minecraft.perks;
        resolve_perk_ids(member_flags, member_id, mc_uuid)
            .filter_map(|id| perks.get(&id))
            .flatten()
            .cloned()
            .collect::<HashSet<String>>()
            .into_iter()
            .collect()
    }
}

// You can get an head icon either from an UUID or name
// Reference: https://minotar.net/
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum HeadIconSource<'a> {
    Username(&'a str),
    Uuid(Uuid),
}

fn resolve_perk_ids(
    member_flags: MemberFlags,
    member_id: Option<Id<UserMarker>>,
    mc_uuid: Option<Uuid>,
) -> impl Iterator<Item = PerkId> {
    let base = std::iter::empty()
        .chain(member_id.map(PerkId::Discord))
        .chain(mc_uuid.map(PerkId::Uuid));

    let roles = member_flags.iter().map(|flag| match flag {
        MemberFlags::ADMINISTRATOR => PerkId::Admins,
        MemberFlags::CONTRIBUTOR => PerkId::Contributors,
        MemberFlags::STAFF => PerkId::Staff,
        flag => unimplemented!("unimplemented flag for {flag:?}"),
    });

    base.chain(roles)
        .chain(member_flags.is_empty().then_some(PerkId::Members))
}

#[cfg(test)]
mod tests {
    use eden_config::{Config, types::organization::minecraft::PerkId};
    use eden_model::tables::member_view::MemberFlags;
    use std::{path::Path, str::FromStr, sync::Arc};
    use twilight_model::id::Id;
    use uuid::Uuid;

    use crate::minecraft::{McService, resolve_perk_ids};

    fn load_config(toml: &str) -> Config {
        Config::maybe_toml_file(toml, Path::new("eden.toml"))
            .unwrap()
            .0
    }

    #[test]
    fn should_resolve_perks_from_multiple_sources() {
        let config = load_config(
            r#"
        [organization.minecraft.perks]
        admins = ["a", "b", "s"]
        staff = ["b"]
        contributors = ["c"]
        members = ["d"]
        "#,
        );

        let mc = McService::new(Arc::new(config));

        let flags = MemberFlags::ADMINISTRATOR | MemberFlags::CONTRIBUTOR;
        let perks = mc.resolve_perks(flags, None, None);
        assert_eq!(perks.len(), 4);

        let flags = MemberFlags::ADMINISTRATOR | MemberFlags::STAFF;
        let perks = mc.resolve_perks(flags, None, None);
        assert_eq!(perks.len(), 3);
    }

    #[test]
    fn should_resolve_specific_perks_for_specific_users() {
        let uuid = Uuid::from_str("74483d8a-e072-4222-9eb9-d1bd3c17fd45").unwrap();
        let config = load_config(
            r#"
        [organization.minecraft.perks]
        "1234" = ["p1_perk"]
        74483d8a-e072-4222-9eb9-d1bd3c17fd45 = ["p2_perk"]
        "#,
        );

        let mc = McService::new(Arc::new(config));

        // Case #1: Discord snowflakes
        let perks = mc.resolve_perks(MemberFlags::empty(), Some(Id::new(1234)), None);
        assert_eq!(perks, &["p1_perk"]);

        // Case #2: UUIDs
        let perks = mc.resolve_perks(MemberFlags::empty(), None, Some(uuid));
        assert_eq!(perks, &["p2_perk"]);
    }

    #[test]
    fn should_resolve_specific_perks_for_designated_roles() {
        let config = load_config(
            r#"
        [organization.minecraft.perks]
        admins = ["admin_perk"]
        staff = ["staff_perk"]
        contributors = ["contrib_perk"]
        members = ["member_perk"]
        "#,
        );

        let mc = McService::new(Arc::new(config));

        // Case #1: Administrators
        let flags = MemberFlags::ADMINISTRATOR;
        let perks = mc.resolve_perks(flags, None, None);
        assert_eq!(perks, &["admin_perk"]);

        // Case #2: Staff
        let flags = MemberFlags::STAFF;
        let perks = mc.resolve_perks(flags, None, None);
        assert_eq!(perks, &["staff_perk"]);

        // Case #3: Contributors
        let flags = MemberFlags::CONTRIBUTOR;
        let perks = mc.resolve_perks(flags, None, None);
        assert_eq!(perks, &["contrib_perk"]);

        // Case #4: Regular members
        let flags = MemberFlags::empty();
        let perks = mc.resolve_perks(flags, None, None);
        assert_eq!(perks, &["member_perk"]);
    }

    #[test]
    fn should_resolve_perk_id_for_admins() {
        let flags = MemberFlags::ADMINISTRATOR;
        let result = resolve_perk_ids(flags, None, None).collect::<Vec<_>>();
        assert_eq!(result, &[PerkId::Admins]);
    }

    #[test]
    fn should_resolve_perk_id_for_staff() {
        let flags = MemberFlags::STAFF;
        let result = resolve_perk_ids(flags, None, None).collect::<Vec<_>>();
        assert_eq!(result, &[PerkId::Staff]);
    }

    #[test]
    fn should_resolve_perk_id_for_contributors() {
        let flags = MemberFlags::CONTRIBUTOR;
        let result = resolve_perk_ids(flags, None, None).collect::<Vec<_>>();
        assert_eq!(result, &[PerkId::Contributors]);
    }

    #[test]
    fn should_resolve_perk_id_for_regular_members() {
        let flags = MemberFlags::REGULAR;
        let result = resolve_perk_ids(flags, None, None).collect::<Vec<_>>();
        assert_eq!(result, &[PerkId::Members]);
    }
}
