use bon::Builder;
use eden_config::LiveConfig;
use eden_metrics::MetricsAdapter;
use eden_services::{Cache, DatabasePools};
use eden_signals::ShutdownSignal;
use std::sync::Arc;

#[derive(Builder, Debug)]
#[builder(finish_fn(name = "build_inner", vis = ""))]
pub struct WebContext {
    pub cache: Arc<dyn Cache>,
    pub config: LiveConfig,
    pub metrics: Option<Arc<dyn MetricsAdapter>>,
    pub pools: DatabasePools,
    pub shutdown_signal: ShutdownSignal,
}

impl<S: web_context_builder::State> WebContextBuilder<S> {
    /// Creates a new [`JobContext`] and wraps it in an [`Arc`] for shared ownership.
    #[must_use]
    pub fn build(self) -> Arc<WebContext>
    where
        S: web_context_builder::IsComplete,
    {
        Arc::new(self.build_inner())
    }
}
