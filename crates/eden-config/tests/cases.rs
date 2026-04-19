use eden_config::{Config, EditableConfig};
use eden_file_diagnostics::RenderedDiagnostic;
use insta::{assert_debug_snapshot, assert_snapshot};
use std::{io::Write, path::Path};
use tempfile::NamedTempFile;

use crate::common::run_case_folder;

mod common;

#[test]
fn test_template_generation() {
    eden_test_util::disable_fancy_error_output();

    let mut settings = insta::Settings::clone_current();
    let path = Path::new("./tests/cases/template").canonicalize().unwrap();
    settings.set_prepend_module_to_snapshot(false);
    settings.set_snapshot_path(&path);
    settings.set_input_file(&path);

    settings.bind(|| {
        let template = Config::template();
        assert_snapshot!("output", template);
    });
}

#[test]
fn test_migration_pass_cases() {
    eden_test_util::disable_fancy_error_output();
    run_case_folder("./tests/cases/migration/pass", |path| {
        let original = eden_paths::read(&path.join(Config::FILE_NAME)).unwrap();

        let mut tempfile = NamedTempFile::new().unwrap();
        tempfile.write_all(original.as_bytes()).unwrap();

        let mut editable = EditableConfig::new(tempfile.path());
        editable.reload().unwrap();

        let schema_version = editable.schema_version();
        editable.perform_migrations().unwrap();

        assert_debug_snapshot!("schema", schema_version);
        assert_snapshot!("new_document", editable.document().raw());

        let config = match editable.parse() {
            Ok(inner) => inner,
            Err(error) => panic!("migration case failed for {path:?}: {error:?}"),
        };

        assert_debug_snapshot!("config", config);
    });
}

#[test]
fn test_migration_fail_cases() {
    eden_test_util::disable_fancy_error_output();
    run_case_folder("./tests/cases/migration/fail", |path| {
        let original = eden_paths::read(&path.join(Config::FILE_NAME)).unwrap();

        let mut tempfile = NamedTempFile::new().unwrap();
        tempfile.write_all(original.as_bytes()).unwrap();

        let mut editable = EditableConfig::new(tempfile.path());
        editable.reload().unwrap();

        let Err(error) = editable.perform_migrations() else {
            panic!("migration case passed for {path:?}");
        };

        // Redacting the temporary path
        let error = format!("{error:#?}").replace(tempfile.path().to_str().unwrap(), "<redacted>");
        assert_snapshot!("error", error);
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
        assert_debug_snapshot!("output", config);
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

        let report = result.unwrap_err();
        let Some(diagnostic) = report.downcast_ref::<RenderedDiagnostic>() else {
            panic!("RenderedDiagnostic should be attached internally")
        };

        let diagnostic = diagnostic
            .clone()
            .into_string()
            .replace(&*path.to_string_lossy(), "");

        assert_snapshot!("error", diagnostic);
    });
}
