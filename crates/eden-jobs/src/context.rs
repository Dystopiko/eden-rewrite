use bon::Builder;
use eden_config::LiveConfig;
use eden_metrics::MetricsAdapter;
use eden_services::AppContext;
use eden_signals::ShutdownSignal;
use std::sync::Arc;

#[derive(Builder, Debug)]
#[builder(finish_fn(name = "build_inner", vis = ""))]
pub struct JobContext {
    pub app: Arc<AppContext>,
    pub config: LiveConfig,
    pub metrics: Option<Arc<dyn MetricsAdapter>>,
    pub shutdown_signal: ShutdownSignal,
}

impl<S: job_context_builder::State> JobContextBuilder<S> {
    /// Creates a new [`JobContext`] and wraps it in an [`Arc`] for shared ownership.
    #[must_use]
    pub fn build(self) -> Arc<JobContext>
    where
        S: job_context_builder::IsComplete,
    {
        Arc::new(self.build_inner())
    }
}
