use twilight_model::id::{Id, marker::UserMarker};
use uuid::Uuid;

use crate::{
    mc_edition::McEdition,
    model::{
        linked_mc_account::{LinkMcAccount, LinkedMcAccount},
        member::NewMember,
    },
};

#[bon::builder]
pub async fn member_with_linked_mc_account(
    discord_user_id: Id<UserMarker>,
    name: &str,
    mc_username: &str,
    #[builder(default = McEdition::Java)] mc_edition: McEdition,
    conn: &mut eden_postgres::Connection,
) -> LinkedMcAccount {
    NewMember::builder()
        .discord_user_id(discord_user_id)
        .name(name)
        .build()
        .upsert(conn)
        .await
        .unwrap();

    LinkMcAccount::builder()
        .uuid(Uuid::new_v4())
        .username(mc_username)
        .edition(mc_edition)
        .member_id(discord_user_id)
        .build()
        .insert(conn)
        .await
        .unwrap()
}

/// Initializes test environment and configures insta snapshot testing settings.
///
/// It returns [`SettingsBindDropGuard`] that maintains the snapshot settings
/// for the scope in which it's held. The settings are automatically reset when
/// the guard is dropped.
///
/// [`SettingsBindDropGuard`]: insta::internals::SettingsBindDropGuard
pub fn setup() -> insta::internals::SettingsBindDropGuard {
    use std::path::Path;

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let snapshots_dir = manifest_dir.join("./tests/snapshots");
    eden_test_util::init_tracing_for_tests();

    let mut settings = insta::Settings::clone_current();
    let path = Path::new(&snapshots_dir).canonicalize().unwrap();
    settings.set_prepend_module_to_snapshot(false);
    settings.set_snapshot_path(&path);
    settings.set_input_file(&path);

    settings.bind_to_scope()
}
