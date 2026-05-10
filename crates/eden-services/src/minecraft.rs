use eden_config::types::organization::{Minecraft, minecraft::PerkId};
use eden_model::tables::member_view::MemberFlags;
use std::collections::HashSet;
use twilight_model::id::{Id, marker::UserMarker};
use uuid::Uuid;

pub fn resolve_perks(
    config: &Minecraft,
    flags: MemberFlags,
    member_id: Option<Id<UserMarker>>,
    uuid: Option<Uuid>,
) -> Vec<String> {
    let perks = &config.perks;
    resolve_perk_ids(flags, member_id, uuid)
        .filter_map(|id| perks.get(&id))
        .flatten()
        .cloned()
        .collect::<HashSet<String>>()
        .into_iter()
        .collect()
}

pub fn resolve_perk_ids(
    flags: MemberFlags,
    member_id: Option<Id<UserMarker>>,
    uuid: Option<Uuid>,
) -> impl Iterator<Item = PerkId> {
    let base = std::iter::empty()
        .chain(member_id.map(PerkId::Discord))
        .chain(uuid.map(PerkId::Uuid));

    let flag_perks = flags.iter().map(|flag| match flag {
        MemberFlags::ADMINISTRATOR => PerkId::Admins,
        MemberFlags::CONTRIBUTOR => PerkId::Contributors,
        MemberFlags::STAFF => PerkId::Staff,
        flag => unimplemented!("unimplemented flag for {flag:?}"),
    });

    base.chain(flag_perks)
        .chain(flags.is_empty().then_some(PerkId::Members))
}

#[cfg(test)]
mod tests {
    use eden_config::types::organization::{Minecraft, minecraft::PerkId};
    use eden_model::tables::member_view::MemberFlags;
    use twilight_model::id::Id;
    use uuid::Uuid;

    use crate::minecraft::{resolve_perk_ids, resolve_perks};

    #[test]
    fn should_resolve_perks_from_multiple_sources() {
        let mut config = Minecraft {
            ..Default::default()
        };

        let perks = &mut config.perks;
        perks.insert(PerkId::Admins, vec!["a".into(), "b".into(), "s".into()]);
        perks.insert(PerkId::Staff, vec!["b".into()]);
        perks.insert(PerkId::Contributors, vec!["c".into()]);
        perks.insert(PerkId::Members, vec!["d".into()]);

        let perks = resolve_perks(
            &config,
            MemberFlags::ADMINISTRATOR | MemberFlags::CONTRIBUTOR,
            None,
            None,
        );
        assert_eq!(perks.len(), 4);

        let perks = resolve_perks(
            &config,
            MemberFlags::ADMINISTRATOR | MemberFlags::STAFF,
            None,
            None,
        );
        assert_eq!(perks.len(), 3);
    }

    #[test]
    fn should_resolve_specific_perks_for_specific_users() {
        let uuid = Uuid::new_v4();

        let mut config = Minecraft {
            ..Default::default()
        };

        let perks = &mut config.perks;
        perks.insert(PerkId::Discord(Id::new(1234)), vec!["p1_perk".into()]);
        perks.insert(PerkId::Uuid(uuid), vec!["p2_perk".into()]);

        // Case #1: Discord snowflakes
        let perks = resolve_perks(&config, MemberFlags::empty(), Some(Id::new(1234)), None);
        assert_eq!(perks, &["p1_perk"]);

        // Case #2: UUIDs
        let perks = resolve_perks(&config, MemberFlags::empty(), None, Some(uuid));
        assert_eq!(perks, &["p2_perk"]);
    }

    #[test]
    fn should_resolve_specific_perks_for_designated_roles() {
        let mut config = Minecraft {
            ..Default::default()
        };

        let perks = &mut config.perks;
        perks.insert(PerkId::Admins, vec!["admin_perk".into()]);
        perks.insert(PerkId::Staff, vec!["staff_perk".into()]);
        perks.insert(PerkId::Contributors, vec!["contrib_perk".into()]);
        perks.insert(PerkId::Members, vec!["member_perk".into()]);

        // Case #1: Administrators
        let flags = MemberFlags::ADMINISTRATOR;
        let perks = resolve_perks(&config, flags, None, None);
        assert_eq!(perks, &["admin_perk"]);

        // Case #2: Staff
        let flags = MemberFlags::STAFF;
        let perks = resolve_perks(&config, flags, None, None);
        assert_eq!(perks, &["staff_perk"]);

        // Case #3: Contributors
        let flags = MemberFlags::CONTRIBUTOR;
        let perks = resolve_perks(&config, flags, None, None);
        assert_eq!(perks, &["contrib_perk"]);

        // Case #4: Regular members
        let flags = MemberFlags::empty();
        let perks = resolve_perks(&config, flags, None, None);
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
