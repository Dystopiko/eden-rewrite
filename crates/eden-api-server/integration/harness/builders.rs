use bon::Builder;
use eden_background_worker::runner::Runner;
use eden_common::{
    AppContext, DatabasePools,
    domain::{Cache, DiscordClient, Notifier},
    infra::{NopCache, Notifiers},
};
use eden_config::{
    Config, LiveConfig,
    types::{
        Token,
        organization::{Discord, minecraft::PerkId},
    },
};
use eden_jobs::{JobContext, RunnerExt};
use eden_metrics::MetricsAdapter;
use eden_postgres::Pool;
use eden_signals::ShutdownSignal;
use heck::ToSnakeCase;
use indexmap::IndexMap;
use std::{path::Path, sync::Arc};
use twilight_model::id::{Id, marker::GuildMarker};

use super::TestHarness;

/// Builder for [`TestHarness`]. Obtain one via [`TestHarness::builder`].
///
/// All `with_*` methods are optional. Sensible defaults are applied for
/// anything not provided:
/// - Cache: [`NopCache`]
/// - Config: empty TOML (all fields at their defaults)
/// - Discord, metrics: disabled
/// - Job runner: not started
#[must_use = "builders do nothing unless you call .build()"]
pub struct TestHarnessBuilder {
    with_runner: bool,
    cache: Arc<dyn Cache>,
    config: Option<Config>,
    discord: Option<Arc<dyn DiscordClient>>,
    metrics: Option<Arc<dyn MetricsAdapter>>,
    notifiers: Notifiers,
    pool: Pool,
}

#[allow(unused)]
impl TestHarnessBuilder {
    pub(super) fn new(pool: Pool) -> Self {
        Self {
            with_runner: false,
            cache: Arc::new(NopCache),
            config: None,
            discord: None,
            metrics: None,
            notifiers: Notifiers::new(),
            pool,
        }
    }

    /// Overrides the default [`NopCache`] with a custom cache implementation.
    pub fn with_cache(mut self, cache: impl Cache) -> Self {
        self.cache = Arc::new(cache);
        self
    }

    /// Overrides the default empty config with a raw config.
    pub fn with_config_str(mut self, config: &str) -> Self {
        let config = Config::maybe_toml_file(config, Path::new(""))
            .expect("failed to parse default config")
            .0;

        self.config = Some(config);
        self
    }

    /// Overrides the default empty config with a parsed [`Config`].
    pub fn with_config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    /// Overrides the default empty config with a closure.
    pub fn with_config_with<F>(mut self, closure: F) -> Self
    where
        F: FnOnce(&mut Config),
    {
        let config = self.config.get_or_insert_with(default_config);
        closure(config);
        self
    }

    /// Registers a Discord client. Without this, Discord-dependent code
    /// will have no client available.
    pub fn with_discord_client(mut self, client: impl DiscordClient) -> Self {
        self.discord = Some(Arc::new(client));
        self
    }

    /// Registers a metrics adapter.
    pub fn with_metrics(mut self, metrics: impl MetricsAdapter + 'static) -> Self {
        self.metrics = Some(Arc::new(metrics));
        self
    }

    /// Overrides the organization configuration from config.
    pub fn with_organization(mut self, org: OrganizationSetup) -> Self {
        self.with_config_with(|config| {
            config.organization.identifier = org
                .identifier
                .as_deref()
                .map(|v| v.to_string())
                .unwrap_or_else(|| org.name.to_snake_case());

            config.organization.name = org.name;
            config.organization.minecraft.perks = org.perks;

            let discord = config.organization.discord.get_or_insert_with(|| Discord {
                token: Token::new("<invalid.discord.token>"),
                guild_id: org.discord_guild_id,
                ids: Default::default(),
                swearing_police: Default::default(),
            });
            discord.guild_id = org.discord_guild_id;
        })
    }

    /// Registers an additional notifier. May be called multiple times.
    pub fn with_notifier(mut self, notifier: impl Notifier) -> Self {
        self.notifiers.add(Arc::new(notifier));
        self
    }

    /// Enables the background job runner. Required to call
    /// [`TestHarness::run_pending_jobs`].
    pub fn with_runner(mut self) -> Self {
        self.with_runner = true;
        self
    }

    /// Consumes the builder and returns a fully initialized [`TestHarness`].
    pub fn build(self) -> TestHarness {
        let metrics = self.metrics;
        let pools = build_pools(self.pool, metrics.clone());
        let app = build_app_context(
            self.cache,
            self.config,
            self.discord,
            metrics,
            self.notifiers,
            pools,
        );

        let runner = self.with_runner.then(|| build_runner(app.clone()));
        eden_test_util::init_tracing_for_tests();

        TestHarness { app, runner }
    }
}

/// Builder for organization setup with [`TestHarness`].
#[derive(Builder)]
pub struct OrganizationSetup {
    #[builder(field)]
    pub perks: IndexMap<PerkId, Vec<String>>,
    pub discord_guild_id: Id<GuildMarker>,
    #[builder(into, default = "Dystopia")]
    pub name: String,
    #[builder(into)]
    pub identifier: Option<String>,
}

impl<S> OrganizationSetupBuilder<S>
where
    S: organization_setup_builder::State,
{
    pub fn perks(mut self, id: PerkId, perks: &[&str]) -> OrganizationSetupBuilder<S> {
        let iter = perks.iter().map(|v| v.to_string());
        self.perks.insert(id, iter.collect::<Vec<_>>());
        self
    }
}

fn build_pools(pool: Pool, metrics: Option<Arc<dyn MetricsAdapter>>) -> DatabasePools {
    DatabasePools::builder()
        .primary_db(pool)
        .maybe_metrics(metrics)
        .build()
}

fn build_app_context(
    cache: Arc<dyn Cache>,
    config: Option<Config>,
    discord: Option<Arc<dyn DiscordClient>>,
    metrics: Option<Arc<dyn MetricsAdapter>>,
    notifiers: Notifiers,
    pools: DatabasePools,
) -> Arc<AppContext> {
    AppContext::builder()
        .cache(cache)
        .config(LiveConfig::new(config.unwrap_or_else(default_config)))
        .maybe_discord(discord)
        .maybe_metrics(metrics)
        .notifier(Arc::new(notifiers))
        .pools(pools)
        .shutdown_signal(ShutdownSignal::new())
        .build()
}

fn build_runner(app: Arc<AppContext>) -> Runner<Arc<JobContext>> {
    let ctx = JobContext::builder().app(app).build();
    let pool = ctx.pools().primary_db().clone();
    Runner::new(ctx, pool)
        .register_eden_job_types()
        .shutdown_when_queue_empty()
}

fn default_config() -> Config {
    Config::maybe_toml_file("", Path::new(""))
        .expect("failed to parse default config")
        .0
}
