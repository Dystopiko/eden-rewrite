use xshell::Shell;

/// Shared runtime context passed to `xtask` subcommands.
#[derive(Debug)]
pub struct RunContext {
    /// Initialized [`Shell`] instance set to the workspace root directory.
    pub sh: Shell,
}
