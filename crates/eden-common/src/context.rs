use crate::{
    CachedRepository, DatabasePools,
    domain::{Cache, DiscordClient, Notifier},
    job_queue::BackgroundJobQueue,
    minecraft::McService,
};

use bon::Builder;
use eden_config::{Config, LiveConfig};
use eden_metrics::MetricsAdapter;
use eden_signals::ShutdownSignal;
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
    discord: Arc<dyn DiscordClient>,
    metrics: Option<Arc<dyn MetricsAdapter>>,
    notifier: Arc<dyn Notifier>,
    pools: DatabasePools,
    shutdown_signal: ShutdownSignal,
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
    pub fn discord(&self) -> Arc<dyn DiscordClient> {
        self.discord.clone()
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
