use eden_common::{
    AppContext, DatabasePools,
    domain::{Cache, DiscordClient, Notifier, discord::MockDiscordClient},
    infra::{MultiPlatformNotifier, NopCache},
};
use eden_config::{Config, LiveConfig};
use eden_metrics::MetricsAdapter;
use eden_postgres::Pool;
use eden_signals::ShutdownSignal;
use std::{path::Path, sync::Arc};

#[must_use = "builders do not do anything unless you build them"]
pub struct MockEdenSystemBuilder {
    pub(super) build_job_runner: bool,
    cache: Arc<dyn Cache>,
    config: Option<Config>,
    discord_client: Option<Arc<dyn DiscordClient>>,
    metrics: Option<Arc<dyn MetricsAdapter>>,
    notifier: MultiPlatformNotifier,
    pool: Pool,
}

impl MockEdenSystemBuilder {
    pub(super) fn new(pool: sqlx::PgPool) -> Self {
        Self {
            build_job_runner: false,
            cache: Arc::new(NopCache),
            config: None,
            discord_client: None,
            metrics: None,
            notifier: MultiPlatformNotifier::new(),
            pool: pool.into(),
        }
    }

    pub fn add_notifier(mut self, notifier: impl Notifier) -> Self {
        self.notifier.add_platform(Arc::new(notifier));
        self
    }

    pub fn with_cache(mut self, cache: impl Cache) -> Self {
        self.cache = Arc::new(cache);
        self
    }

    pub fn with_config(mut self, config: &str) -> Self {
        let (config, _) = Config::maybe_toml_file(config, Path::new(""))
            .expect("failed to parse custom config from source");

        self.config = Some(config);
        self
    }

    pub fn with_discord_client(mut self, service: impl DiscordClient + 'static) -> Self {
        self.discord_client = Some(Arc::new(service));
        self
    }

    pub fn with_metrics(mut self, metrics: impl MetricsAdapter + 'static) -> Self {
        self.metrics = Some(Arc::new(metrics));
        self
    }

    pub fn with_runner(mut self) -> Self {
        self.build_job_runner = true;
        self
    }
}

impl MockEdenSystemBuilder {
    pub(super) fn build_app_context(self, shutdown_signal: ShutdownSignal) -> Arc<AppContext> {
        let pools = DatabasePools::builder()
            .primary_db(self.pool.clone())
            .maybe_metrics(self.metrics.clone())
            .build();

        AppContext::builder()
            .cache(self.cache.clone())
            .config(LiveConfig::new(self.resolve_config()))
            .maybe_discord(self.discord_client)
            .maybe_metrics(self.metrics)
            .notifier(Arc::new(self.notifier))
            .pools(pools)
            .shutdown_signal(shutdown_signal)
            .build()
    }

    fn resolve_config(&self) -> Config {
        self.config.clone().unwrap_or_else(|| {
            Config::maybe_toml_file(
                r#"
                [organization.discord]
                guild_id = "123456789"            
                token = "foo"
                "#,
                Path::new(""),
            )
            .expect("failed to parse default config")
            .0
        })
    }
}
