/// Initializes test environment and configures insta snapshot testing settings.
///
/// It returns [`SettingsBindDropGuard`] that maintains the snapshot settings
/// for the scope in which it's held. The settings are automatically reset when
/// the guard is dropped.
///
/// [`SettingsBindDropGuard`]: insta::internals::SettingsBindDropGuard
pub fn setup(path: &[&str]) -> insta::internals::SettingsBindDropGuard {
    use std::path::Path;

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut snapshots_dir = manifest_dir.join("./tests/snapshots");
    for descendant in path {
        snapshots_dir = snapshots_dir.join(descendant);
    }

    std::fs::create_dir_all(&snapshots_dir).unwrap();

    let mut settings = insta::Settings::clone_current();
    let path = Path::new(&snapshots_dir).canonicalize().unwrap();
    settings.set_prepend_module_to_snapshot(false);
    settings.set_snapshot_path(&path);
    settings.set_input_file(&path);

    settings.bind_to_scope()
}
