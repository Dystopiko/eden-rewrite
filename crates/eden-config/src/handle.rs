use arc_swap::ArcSwap;
use eden_config_types::Config;
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Clone)]
pub struct ConfigHandle {
    pointer: Arc<ArcSwap<Config>>,
    pub(crate) watch: watch::Sender<Arc<Config>>,
}

impl ConfigHandle {
    #[must_use]
    pub fn new(inner: Config) -> Self {
        let inner = Arc::new(inner);
        let pointer = Arc::new(ArcSwap::new(inner.clone()));
        let (watch, _) = watch::channel(inner);

        Self { pointer, watch }
    }

    #[must_use]
    pub fn get(&self) -> Arc<Config> {
        self.pointer.load_full()
    }

    pub fn update(&self, value: Config) {
        let new_value = Arc::new(value);
        self.pointer.store(new_value.clone());
        self.watch.send_replace(new_value);
    }
}
