use crate::{
    CachedRepository, DatabasePools,
    domain::{Cache, DiscordClient, Notifier, System},
    infra::real_system::RealSystem,
    job_queue::BackgroundJobQueue,
    minecraft::McService,
};

use bon::Builder;
use eden_config::{Config, LiveConfig};
use eden_metrics::MetricsAdapter;
use eden_postgres::{Pool, pool::InvalidConnectionUrl};
use eden_signals::ShutdownSignal;
use error_stack::{Report, ResultExt};
use std::sync::Arc;

/// Central container for infrastructure services and shared application
/// dependencies used throughout Eden.
///
/// It owns handles to core services such as caching, database pools,
/// and external integrations. It is initialized once during application
/// startup and cheaply cloned where needed.
#[derive(Builder, Clone, Debug)]
#[builder(finish_fn(name = "build_inner", vis = ""))]
#[must_use]
pub struct AppContext {
    cache: Arc<dyn Cache>,
    config: LiveConfig,
    discord: Option<Arc<dyn DiscordClient>>,
    metrics: Option<Arc<dyn MetricsAdapter>>,
    notifier: Arc<dyn Notifier>,
    pools: DatabasePools,
    shutdown_signal: ShutdownSignal,
    #[builder(default = Arc::new(RealSystem))]
    system: Arc<dyn System>,
}

impl AppContext {
    /// Returns a handle to the cache.
    pub fn cache(&self) -> Arc<dyn Cache> {
        self.cache.clone()
    }

    /// Returns a current pointer to a [`Config`] object. This object
    /// may be changed at any time.
    ///
    /// [`Config`]: eden_config::Config
    #[must_use]
    pub fn config(&self) -> Arc<Config> {
        self.config.get()
    }

    /// Returns a handle to a [`DiscordClient`].
    pub fn discord(&self) -> Option<Arc<dyn DiscordClient>> {
        self.discord.clone()
    }

    /// Returns the raw handle of [`LiveConfig`].
    pub fn live_config(&self) -> LiveConfig {
        self.config.clone()
    }

    /// Returns a reference to a [metrics adapter].
    ///
    /// [metrics adapter]: MetricsAdapter
    pub fn metrics(&self) -> Option<&dyn MetricsAdapter> {
        self.metrics.as_deref()
    }

    /// Returns a handle to a [`Notifier`].
    pub fn notifier(&self) -> Arc<dyn Notifier> {
        self.notifier.clone()
    }

    /// Returns a clone of the database pools.
    pub fn pools(&self) -> DatabasePools {
        self.pools.clone()
    }

    /// Returns a [shutdown signal].
    ///
    /// [shutdown signal]: ShutdownSignal
    pub fn shutdown_signal(&self) -> &ShutdownSignal {
        &self.shutdown_signal
    }

    /// Returns a handler to a [`System`] domain.
    pub fn system(&self) -> Arc<dyn System> {
        self.system.clone()
    }

    /// Creates a background job queue service.
    pub fn job_queue(&self) -> BackgroundJobQueue<'_> {
        BackgroundJobQueue::new(&self.pools)
    }

    /// Creates a [Minecraft service].
    ///
    /// [Minecraft service]: McService
    pub fn minecraft(&self) -> McService {
        McService::new(self.config.get())
    }

    /// Creates a cached repository service.
    pub fn repository(&self) -> CachedRepository<'_> {
        CachedRepository::new(&*self.cache, &self.pools)
    }
}

// type PoolsFromConfigResult<S> =
//     Result<AppContextBuilder<app_context_builder::SetPools<S>>, Report<InvalidConnectionUrl>>;

impl AppContext {
    pub fn pools_from_config(
        config: &Config,
        metrics: Option<Arc<dyn MetricsAdapter>>,
    ) -> Result<DatabasePools, Report<InvalidConnectionUrl>> {
        let primary = Pool::new(&config.database.common, &config.database.primary)
            .attach("while trying to create primary database pool")?;

        let replica = config
            .database
            .replica
            .as_ref()
            .map(|replica| Pool::new(&config.database.common, replica))
            .transpose()
            .attach("while trying to create replica database pool")?;

        let pools = DatabasePools::builder()
            .primary_db(primary)
            .maybe_replica_db(replica)
            .maybe_metrics(metrics.clone())
            .build();

        Ok(pools)
    }
}

impl<S> AppContextBuilder<S>
where
    S: app_context_builder::State,
{
    pub fn build(self) -> Arc<AppContext>
    where
        S: app_context_builder::IsComplete,
    {
        Arc::new(self.build_inner())
    }
}
