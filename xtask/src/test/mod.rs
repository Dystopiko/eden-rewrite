use erased_report::ErasedReport;
use error_stack::ResultExt;
use xshell::Shell;

use crate::flags;

/// Insta version reported to the test runner via `INSTA_CARGO_INSTA_VERSION`.
const INSTA_VERSION: &str = "1.47.2";

impl flags::Test {
    pub(crate) fn run(self, sh: &Shell) -> Result<(), ErasedReport> {
        // Runs the test suite, configuring `insta` snapshot behaviour so that
        // unseen snapshots are silently accepted and per-test summaries are
        // suppressed.
        let cmd = self
            .make_test_runner_cmd(sh)
            .arg("--no-fail-fast")
            // Identify ourselves as cargo-insta so that insta's integration
            // is activated without requiring the binary to be installed.
            .env("INSTA_CARGO_INSTA", "1")
            .env("INSTA_CARGO_INSTA_VERSION", INSTA_VERSION)
            // Accept unseen snapshots automatically and suppress per-test
            // snapshot summaries to keep output readable
            //
            // (if it's not running under CI).
            .env("INSTA_UPDATE", if crate::is_ci() { "no" } else { "new" })
            .env("INSTA_OUTPUT", "none");

        cmd.run().attach("could not perform tests")?;
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
