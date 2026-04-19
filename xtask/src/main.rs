use erased_report::{ErasedReport, IntoErasedReportExt};
use std::path::PathBuf;
use xshell::Shell;

mod flags;
mod test;

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

    let sh = &Shell::new().erase_report()?;
    sh.change_dir(workspace_dir());

    let env = env_logger::Env::default().default_filter_or("info");
    env_logger::Builder::from_env(env)
        .format_module_path(false)
        .format_timestamp(None)
        .init();

    if let Some(dotenv) = dotenv {
        log::debug!("using dotenv file: {}", dotenv.display());
    }

    match flags.subcommand {
        flags::XtaskCmd::Test(cmd) => cmd.run(sh),
    }
}

/// Are we running in in a CI environment?
fn is_ci() -> bool {
    match std::env::var("CI").ok().as_deref() {
        Some("false") | Some("0") | Some("") => false,
        None => std::env::var("TF_BUILD").is_ok(),
        Some(_) => true,
    }
}

/// Returns the path to the cargo executable.
#[must_use]
fn cargo() -> std::ffi::OsString {
    let cargo = std::env::var_os("CARGO");
    let cargo = cargo
        .as_deref()
        .unwrap_or_else(|| std::ffi::OsStr::new("cargo"));

    cargo.to_os_string()
}

/// Returns the path to the root directory of Eden backend repository.
#[must_use]
fn workspace_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_WORKSPACE_DIR"))
}
