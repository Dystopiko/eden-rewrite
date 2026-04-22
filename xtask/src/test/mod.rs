use erased_report::ErasedReport;
use error_stack::ResultExt;
use std::path::Path;
use tempfile::tempdir;
use xshell::Shell;

use crate::flags;

/// Insta version reported to the test runner via `INSTA_CARGO_INSTA_VERSION`.
const INSTA_VERSION: &str = "1.47.2";

impl flags::Test {
    pub(crate) fn run(self, sh: &Shell) -> Result<(), ErasedReport> {
        // Create a new temporary file for our warnings since according to insta:
        // "test runners like nextest suppress output from passing tests by default."
        let temp_dir = tempdir().attach("could not create temporary directory for log file")?;

        // insta's runtime will automatically create a new file if it is missing
        let warnings_file = temp_dir.path().join("insta-warnings");
        self.make_test_runner_cmd(sh)
            .arg("--no-fail-fast")
            .env(
                "EDEN_TEST_PGDATA",
                concat!(env!("CARGO_WORKSPACE_DIR"), "target/pgdata"),
            )
            // Identify ourselves as cargo-insta so that insta's integration
            // is activated without requiring the binary to be installed.
            .env("INSTA_CARGO_INSTA", "1")
            .env("INSTA_CARGO_INSTA_VERSION", INSTA_VERSION)
            // Mirror `cargo insta test` behavior: force-pass outside CI so
            // new snapshots are written rather than failing the run.
            .env("INSTA_FORCE_PASS", "1")
            .env("INSTA_UPDATE", if crate::is_ci() { "no" } else { "new" })
            .env("INSTA_WARNINGS_FILE", &warnings_file)
            .run()
            .attach("could not perform tests")?;

        if process_insta_warnings(&warnings_file) {
            log::error!(
                "New snapshots are stored. Run `cargo insta review` to accept or reject them."
            );
            std::process::exit(1);
        }

        Ok(())
    }

    pub(crate) fn make_test_runner_cmd<'s>(&self, sh: &'s Shell) -> xshell::Cmd<'s> {
        let mut cmd = sh.cmd(crate::cargo());
        if self.wants_nextest() {
            cmd = cmd.arg("nextest").arg("run");
        } else {
            cmd = cmd.arg("test");
        }
        cmd
    }

    /// Returns `true` when `cargo-nextest` should be used as the test runner.
    ///
    /// Nextest is selected when either:
    /// - the `--nextest` flag was passed on the command line, or
    /// - the `EDEN_XTASK_USE_NEXTEST` environment variable is set to `"1"`.
    pub(crate) fn wants_nextest(&self) -> bool {
        let env_opt = eden_env_vars::var("EDEN_XTASK_USE_NEXTEST")
            .ok()
            .flatten()
            .unwrap_or_default();

        self.nextest || env_opt == "1"
    }
}

/// Prints deduplicated insta warnings and returns `true` if any new snapshots were stored.
fn process_insta_warnings(warnings_file: &Path) -> bool {
    if !warnings_file.exists() {
        return false;
    }

    let Ok(contents) = eden_paths::read(warnings_file) else {
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
