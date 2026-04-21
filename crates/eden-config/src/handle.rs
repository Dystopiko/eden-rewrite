use arc_swap::ArcSwap;
use std::sync::Arc;
use tokio::sync::watch;

use crate::root::Config;

/// A thread-safe handle for accessing and updating configuration.
///
/// `LiveConfig` provides a lock-free mechanism for reading configuration
/// and notifying subscribers for updates.
///
/// This type is [`Clone`] and can be safely shared across threads and tasks.
/// All operations are thread-safe and lock-free for reads.
#[derive(Clone)]
pub struct LiveConfig {
    inner: Arc<LiveConfigInner>,
}

impl LiveConfig {
    /// Creates a new configuration handle with the given initial value.
    #[must_use]
    pub fn new(value: Config) -> Self {
        let pointer = Arc::new(value);
        let inner = Arc::new(LiveConfigInner {
            pointer: ArcSwap::new(pointer.clone()),
            watch: watch::channel(pointer).0,
        });
        Self { inner }
    }

    /// Returns a reference-counted pointer to the current configuration.
    ///
    /// This operation is lock-free and extremely fast, making it suitable
    /// for high-frequency access patterns.
    #[must_use]
    pub fn get(&self) -> Arc<Config> {
        self.inner.pointer.load_full()
    }

    /// Creates a new receiver for watching configuration changes.
    ///
    /// The receiver will be notified whenever [`update`](Self::update) is called
    /// with a different configuration value. Multiple receivers can be created
    /// and will all receive notifications independently.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<Arc<Config>> {
        self.inner.watch.subscribe()
    }

    /// Updates the configuration and notifies all subscribers.
    ///
    /// If the new configuration is equal to the current one (using [`PartialEq`]),
    /// the update is skipped and subscribers are not notified. This prevents
    /// unnecessary notifications when the configuration hasn't actually changed.
    ///
    /// # Performance Note
    ///
    /// The equality check involves comparing the entire configuration structure.
    /// For very large configurations, consider if this check is necessary for
    /// your use case.
    pub fn update(&self, value: Config) {
        let new_value = Arc::new(value);

        // Compare using Arc pointers for efficiency when they're the same
        let current = self.inner.pointer.load();
        if Arc::ptr_eq(&current, &new_value) || **current == *new_value {
            return;
        }

        self.inner.pointer.store(new_value.clone());
        let _ = self.inner.watch.send(new_value);
    }
}

impl std::fmt::Debug for LiveConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveConfig")
            .field("config", &*self.get())
            .finish()
    }
}

struct LiveConfigInner {
    pointer: ArcSwap<Config>,
    watch: watch::Sender<Arc<Config>>,
}
