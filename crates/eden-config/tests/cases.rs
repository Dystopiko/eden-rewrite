use std::io::Write;

use eden_config::{Config, EditableConfig};
use insta::assert_debug_snapshot;
use tempfile::NamedTempFile;

use crate::common::run_case_folder;

mod common;

#[test]
fn test_migration_cases() {
    eden_test_util::disable_fancy_error_output();
    run_case_folder("./tests/cases/migration", |path| {
        let original = eden_paths::read(&path.join(Config::FILE_NAME)).unwrap();

        let mut tempfile = NamedTempFile::new().unwrap();
        tempfile.write_all(original.as_bytes()).unwrap();

        let mut editable = EditableConfig::new(tempfile.path());
        editable.reload().unwrap();

        let results = editable.perform_migrations().unwrap();
        assert_debug_snapshot!(results);

        let config = match editable.parse() {
            Ok(inner) => inner,
            Err(error) => panic!("migration case failed for {path:?}: {error:?}"),
        };
        assert_debug_snapshot!("config", config);
    });
}

#[test]
fn test_pass_cases() {
    eden_test_util::disable_fancy_error_output();
    run_case_folder("./tests/cases/pass", |path| {
        let mut editable = EditableConfig::new(path.join(Config::FILE_NAME));
        editable.reload().unwrap();

        let config = match editable.parse() {
            Ok(inner) => inner,
            Err(error) => panic!("pass case failed for {path:?}: {error:?}"),
        };

        assert_debug_snapshot!("error", config);
    });
}

#[test]
fn test_fail_cases() {
    eden_test_util::disable_fancy_error_output();
    run_case_folder("./tests/cases/fail", |path| {
        let mut editable = EditableConfig::new(path.join(Config::FILE_NAME));
        editable.reload().unwrap();

        let result = editable.parse();
        if result.is_ok() {
            panic!("fail case passed for {path:?}");
        }

        assert_debug_snapshot!("error", result.unwrap_err());
    });
}
