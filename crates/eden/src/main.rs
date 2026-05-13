use eden::{
    bootstrap::{init_rustls, init_tracing, load_config},
    sys::is_running_in_container,
};
use eden_background_worker::runner::Runner;
use eden_common::{
    AppContext,
    domain::{discord::MockDiscordClient, notifier::MockNotifier},
    infra::{LocalMemoryCache, MultiPlatformNotifier},
};
use eden_config::LiveConfig;
use eden_discord_bot::infra::{DiscordClientImpl, DiscordNotifier};
use eden_jobs::{JobContext, RunnerExt};
use eden_metrics::Prometheus;
use eden_signals::ShutdownSignal;
use erased_report::ErasedReport;
use error_stack::{Report, ResultExt};
use futures::{FutureExt, TryFutureExt};
use std::{io, sync::Arc};
use thiserror::Error;
use tracing::warn;

#[derive(Debug, Error)]
#[error("could not start Eden")]
struct StartEdenError;

fn main() -> Result<(), Report<StartEdenError>> {
    let dotenv = eden_env_vars::load().ok().flatten();
    init_rustls().change_context(StartEdenError)?;
    init_tracing();

    if let Some(dotenv) = dotenv {
        tracing::debug!("using dotenv file: {}", dotenv.display());
    }

    let config = load_config()
        .change_context(StartEdenError)
        .inspect_err(suggest_permission_fixes)?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .change_context(StartEdenError)
        .attach("could not build tokio runtime")?;

    let result = rt.block_on(async move {
        let shutdown_signal = ShutdownSignal::new();
        let prometheus = Prometheus::new()
            .map(Arc::new)
            .change_context(StartEdenError)?;

        let pools = AppContext::pools_from_config(&config, Some(prometheus.clone()))
            .change_context(StartEdenError)?;

        let mut discord_client = None;
        let mut notifier = MultiPlatformNotifier::new();

        let discord = config
            .organization
            .discord
            .as_ref()
            .map(|inner| {
                let service = eden_discord_bot::service(
                    &config,
                    shutdown_signal.clone(),
                    Some(prometheus.clone()),
                    pools.clone(),
                    inner,
                );

                discord_client = Some(DiscordClientImpl::new(&service.ctx));
                notifier.add_platform(DiscordNotifier::new(&service.ctx));
                service.start().map_err(ErasedReport::from_report).boxed()
            })
            .unwrap_or_else(|| {
                warn!("Discord service is disabled");
                async { Ok(()) }.boxed()
            });

        let app = AppContext::builder()
            .cache(Arc::new(LocalMemoryCache::builder().build()))
            .maybe_discord(discord_client.map(|v| v as Arc<_>))
            .notifier(Arc::new(notifier))
            .metrics(prometheus)
            .pools(pools)
            .config(LiveConfig::new(config))
            .shutdown_signal(shutdown_signal)
            .build();

        let job_context = JobContext::builder().app(app.clone()).build();
        let bg_handle = app.config().background_jobs.enabled.then(|| {
            let primary_pool = app.pools().primary_db().clone();
            Runner::new(job_context, primary_pool)
                .register_eden_job_types()
                .start()
        });

        if bg_handle.is_none() {
            tracing::info!("background job runner is disabled");
        }

        let config = app.config();
        let api_server = if let Some(server) = config.server.as_ref() {
            eden_api_server::service(app.clone(), server)
                .map_err(|report| ErasedReport::from_report(report))
                .boxed()
        } else {
            tracing::warn!("API server service is disabled");
            async { Ok::<(), ErasedReport>(()) }.boxed()
        };

        let shutdown_signal = app.shutdown_signal().clone();
        tokio::spawn(async move {
            let triggered_by = eden::bootstrap::shutdown_signal().await;
            tracing::warn!("received {triggered_by}; initiating graceful shutdown");
            shutdown_signal.initiate();
        });

        let result = tokio::try_join!(api_server, discord);

        app.shutdown_signal().subscribe().await;
        tracing::info!("shutting down Eden");

        if let Some(bg_handle) = bg_handle {
            bg_handle.shutdown().await;
        }

        result.map(|_| ())
    });

    result.map_err(|e| e.change_context(StartEdenError))
}

fn suggest_permission_fixes(report: &Report<StartEdenError>) {
    let Some(error) = report.downcast_ref::<io::Error>() else {
        return;
    };

    // memothelemo:
    //
    // When hosting this Docker container, I encountered permission issues while trying
    // to write to a file because Eden always writes the config file to disk
    // to configure defaults (if not set).
    //
    // I believe this is caused by a file permission conflict when mounting files as volumes
    // between Docker and the host system, so we warn the user to try to set the config file
    // permissions to 1000:1000 first.
    if matches!(error.kind(), io::ErrorKind::PermissionDenied) && is_running_in_container() {
        warn!(
            "Container permission issue detected! Mounted files may have incompatible ownership. \
            To fix this, try running `chown -R 1000:1000 <file>` in the host to align them with \
            the container's expected UID/GID. This typically occurs when Docker volumes inherit \
            incompatible host permissions."
        );
    }
}
