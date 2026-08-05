mod context;
mod flags;
mod test;

use self::context::RunContext;

use erased_report::{EraseReportExt, ErasedReport};
use error_stack::ResultExt;
use std::path::PathBuf;
use xshell::Shell;

fn main() -> Result<(), ErasedReport> {
    let dotenv = eden_env_vars::load().ok().flatten();
    let flags = match flags::Xtask::from_env() {
        Ok(flags) => flags,
        Err(error) if error.is_help() => {
            let error = error
                .to_string()
                .replace("{CARGO_PKG_VERSION}", env!("CARGO_PKG_VERSION"));

            println!("{error}");
            std::process::exit(0);
        }
        Err(error) => error.exit(),
    };

    let sh = Shell::new()
        .attach("could not initialize shell execution context")
        .erase_report()?;

    let ctx = RunContext { sh };
    ctx.sh.change_dir(workspace_dir());

    let env = env_logger::Env::default().default_filter_or("info");
    env_logger::Builder::from_env(env)
        .format_module_path(false)
        .format_timestamp(None)
        .init();

    if let Some(dotenv) = dotenv {
        log::debug!("using dotenv file: {}", dotenv.display());
    }

    match &flags.subcommand {
        flags::XtaskCmd::Test(flags) => self::test::main(&ctx, flags),
    }
}

/// Returns whether execution is running inside a CI environment.
fn is_ci() -> bool {
    match std::env::var("CI").ok().as_deref() {
        Some("false") | Some("0") | Some("") => false,
        None => std::env::var("TF_BUILD").is_ok(),
        Some(_) => true,
    }
}

/// Returns the path or executable name for the `cargo` binary.
#[must_use]
fn cargo() -> std::ffi::OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

/// Returns the path to the root directory of the Eden repository.
#[must_use]
fn workspace_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_WORKSPACE_DIR"))
}
