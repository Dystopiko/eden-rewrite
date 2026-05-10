pub mod test_server;
pub use self::test_server::TestApp;

#[doc(hidden)]
pub fn get_function_name(name: &'static str) -> &'static str {
    name.split("::").last().unwrap_or(name)
}

macro_rules! setup_for_route {
    [ $($path:expr),* ] => {{
        ::eden_test_util::init_tracing_for_tests();
        crate::testing::setup_inner(
            &[ $( $path ),* ,
            crate::testing::get_function_name(::insta::_function_name!())
        ])
    }};
}
pub(super) use setup_for_route;

macro_rules! assert_response {
    ($response:ident) => {{
        let response = &$response;
        ::insta::assert_debug_snapshot!("headers", response.headers());
        ::insta::assert_debug_snapshot!("body", response.as_bytes());

        let status_code = response.status_code();
        let status_str = crate::testing::assert_response!(for status_code = status_code);
        ::insta::assert_snapshot!("status", status_str);
    }};
    ($response:ident as str) => {{
        let response = &$response;
        ::insta::assert_debug_snapshot!("headers", response.headers());
        ::insta::assert_snapshot!("body", String::from_utf8_lossy(response.as_bytes()));

        let status_str = crate::testing::assert_response!(for status_code = response.status_code());
        ::insta::assert_snapshot!("status", status_str);
    }};
    (for status_code = $status_code:expr) => {{
        let mut status_str = $status_code.as_str().to_string();
        if let Some(phrase) = $status_code.canonical_reason() {
            status_str.push_str(" (");
            status_str.push_str(phrase);
            status_str.push(')');
        }
        status_str
    }};
}
pub(super) use assert_response;

/// Initializes test environment and configures insta snapshot testing settings.
///
/// It returns [`SettingsBindDropGuard`] that maintains the snapshot settings
/// for the scope in which it's held. The settings are automatically reset when
/// the guard is dropped.
///
/// [`SettingsBindDropGuard`]: insta::internals::SettingsBindDropGuard
#[doc(hidden)]
pub fn setup_inner(path: &[&str]) -> insta::internals::SettingsBindDropGuard {
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
