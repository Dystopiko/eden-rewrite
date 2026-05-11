use crate::{
    CachedRepository, DatabasePools,
    domain::{Cache, DiscordClient, Notifier},
    job_queue::BackgroundJobQueue,
    minecraft::McService,
};

use eden_config::LiveConfig;
use std::sync::Arc;

/// Central container for infrastructure services and shared application
/// dependencies used throughout Eden.
///
/// It owns handles to core services such as caching, database pools,
/// and external integrations. It is initialized once during application
/// startup and cheaply cloned where needed.
#[derive(Clone, Debug)]
pub struct AppContext {
    cache: Arc<dyn Cache>,
    config: LiveConfig,
    discord: Arc<dyn DiscordClient>,
    notifier: Arc<dyn Notifier>,
    pools: DatabasePools,
}

impl AppContext {
    /// Creates a new service provider with the given dependencies.
    pub fn new(
        cache: Arc<dyn Cache>,
        config: LiveConfig,
        notifier: Arc<dyn Notifier>,
        pools: DatabasePools,
        discord: Arc<dyn DiscordClient>,
    ) -> Arc<Self> {
        Arc::new(Self {
            cache,
            config,
            notifier,
            pools,
            discord,
        })
    }

    /// Returns a handle to the cache.
    pub fn cache(&self) -> Arc<dyn Cache> {
        self.cache.clone()
    }

    /// Returns a clone of the database pools.
    pub fn pools(&self) -> DatabasePools {
        self.pools.clone()
    }

    /// Returns a handle to a [`DiscordClient`].
    pub fn discord(&self) -> Arc<dyn DiscordClient> {
        self.discord.clone()
    }

    /// Returns a handle to a [`Notifier`].
    pub fn notifier(&self) -> Arc<dyn Notifier> {
        self.notifier.clone()
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
