use eden_test_harness::database::prepare_template_db;
use erased_report::{EraseReportExt, ErasedReport};
use error_stack::ResultExt;
use log::{debug, error, warn};
use std::{fs, path::Path};
use tempfile::tempdir;

use crate::{context::RunContext, flags};

/// Insta version reported to the test runner via `INSTA_CARGO_INSTA_VERSION`.
const INSTA_VERSION: &str = "1.47.2";

/// Runs the test command for the workspace or a specified crate.
pub fn main(ctx: &RunContext, flags: &flags::Test) -> Result<(), ErasedReport> {
    let should_use_nextest = should_use_nextest(flags);
    let tempdir = tempdir()
        .attach("could not create temporary directory for log file")
        .erase_report()?;

    let database_url = eden_env_vars::required_var_parsed("DATABASE_URL")
        .attach("DATABASE_URL is required to perform tests for most of the test cases")
        .erase_report()?;

    debug!("preparing template DDL file...");

    let mut cmd = ctx.sh.cmd(crate::cargo());
    let ddl_file = prepare_template_db(&database_url)?;
    let warnings_file = tempdir.path().join("insta-warnings");

    if should_use_nextest {
        cmd = cmd.arg("nextest").arg("run");
    } else {
        cmd = cmd.arg("test");
    }

    // Identify ourselves as cargo-insta so that insta's snapshot integration is activated
    // without requiring the cargo-insta binary to be installed on the system.
    cmd = cmd
        .arg("--no-fail-fast")
        .env("EDEN_TEST_DB_DDL_PATH", &ddl_file)
        .env("EDEN_TEST_DB_URL", database_url.as_str())
        .env("INSTA_CARGO_INSTA", "1")
        .env("INSTA_CARGO_INSTA_VERSION", INSTA_VERSION)
        // Mirror `cargo insta test` behavior: force-pass outside CI so new
        // snapshots are written rather than failing the run.
        .env("INSTA_FORCE_PASS", "1")
        .env("INSTA_UPDATE", if crate::is_ci() { "no" } else { "new" })
        .env("INSTA_WARNINGS_FILE", &warnings_file);

    if let Some(krate) = flags.krate.as_deref() {
        cmd = cmd.args(&["-p", krate]);
    } else {
        cmd = cmd.arg("--all");
    }

    let result = cmd.run().attach("could not perform tests").erase_report();

    // Remove the ddl file after test execution before throwing
    // an error from the test runner.
    _ = fs::remove_file(ddl_file);
    result?;

    if process_insta_warnings(&warnings_file) {
        error!("New snapshots are stored. Run `cargo insta review` to accept or reject them.");
        std::process::exit(1);
    }

    Ok(())
}

/// Returns `true` if the test command should use `cargo-nextest` by checking
/// its installation, `--nextest` flag and `EDEN_XTASK_USE_NEXTEST`.
fn should_use_nextest(flags: &flags::Test) -> bool {
    let wants_nextest = eden_env_vars::required_var("EDEN_XTASK_USE_NEXTEST")
        .is_ok_and(|v| v == "1")
        || flags.nextest;

    if !wants_nextest {
        return false;
    }

    if has_nextest() {
        true
    } else {
        warn!("cargo-nextest was requested but is not installed; falling back to cargo test");
        false
    }
}

/// Returns `true` if `cargo-nextest` is installed and functional on the host system.
fn has_nextest() -> bool {
    let mut cmd = std::process::Command::new(crate::cargo());
    cmd.arg("nextest").arg("--version");
    cmd.output().is_ok_and(|v| v.status.success())
}

/// Prints deduplicated insta warnings and returns `true` if any new snapshots were stored.
fn process_insta_warnings(warnings_file: &Path) -> bool {
    if !warnings_file.exists() {
        return false;
    }

    let Ok(contents) = fs::read_to_string(warnings_file) else {
        return false;
    };

    let mut seen = std::collections::BTreeSet::new();
    let mut has_new_snapshots = false;

    for line in contents.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if seen.insert(line.to_owned()) {
            eprintln!("{line}");
            has_new_snapshots |= line.contains("stored new snapshot");
        }
    }

    has_new_snapshots
}
