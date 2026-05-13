use eden_config::{Config, EditableConfig, editable::LoadConfigError};
use error_stack::{Report, ResultExt};
use std::path::Path;
use thiserror::Error;
use tokio::signal::unix::{SignalKind, signal};
use tracing::{level_filters::LevelFilter, warn};
use tracing_subscriber::{EnvFilter, Layer, prelude::*};

/// Error returned when the rustls crypto provider cannot be installed.
#[derive(Debug, Error)]
#[error("Failed to initialize rustls crypto provider")]
pub struct InitRustlsError;

pub fn load_config() -> Result<Config, Report<LoadConfigError>> {
    let Some(path) = Config::find() else {
        let template = Config::template();
        let path = Path::new(Config::FILE_NAME);
        eden_paths::write_atomic(path, template)
            .change_context(LoadConfigError)
            .attach("while tryin to save template config file")?;

        warn!(
            "No config file found! Wrote template config file at: {}",
            path.display()
        );
        warn!("Please edit this config file to configure Eden then re-run.");

        std::process::exit(1);
    };

    tracing::debug!("using config file: {}", path.display());

    let mut config = EditableConfig::new(&path);
    config.reload()?;
    config.edit(|_| {}).change_context(LoadConfigError)?;

    let config = config.parse()?;
    tracing::debug!(?config);

    Ok(config)
}

/// Installs the default [`ring`] crypto provider for [`rustls`].
///
/// **This function must be called preferably before Eden binary starts.**
///
/// [`ring`]: rustls::crypto::ring
pub fn init_rustls() -> Result<(), Report<InitRustlsError>> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| Report::new(InitRustlsError))
}

/// Initializes the global [`tracing`] subscriber for console logging.
pub fn init_tracing() {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    let fmt_layer = tracing_subscriber::fmt::layer()
        .without_time()
        .with_filter(env_filter);

    // let sentry_layer = sentry::integrations::tracing::layer()
    //     .enable_span_attributes()
    //     .with_filter(LevelFilter::INFO);

    tracing_subscriber::registry()
        .with(fmt_layer)
        // .with(sentry_layer)
        .init();
}

/// Waits for an OS-level shutdown signal and returns the signal name
/// when one is received.
///
/// | Platform | Signals handled        |
/// |----------|------------------------|
/// | Unix     | `SIGINT`, `SIGTERM`    |
///
/// This function is intended to be used as a graceful-shutdown trigger.
#[must_use]
pub async fn shutdown_signal() -> &'static str {
    let mut sigint = signal(SignalKind::interrupt()).expect("failed to install SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");

    tokio::select! {
        _ = sigint.recv() => "SIGINT",
        _ = sigterm.recv() => "SIGTERM",
    }
}
