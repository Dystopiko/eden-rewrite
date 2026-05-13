use bon::Builder;
use eden_common::AppContext;
use std::sync::Arc;

#[derive(Builder, Debug)]
#[builder(finish_fn(name = "build_inner", vis = ""))]
pub struct JobContext {
    app: Arc<AppContext>,
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

impl std::ops::Deref for JobContext {
    type Target = AppContext;

    fn deref(&self) -> &Self::Target {
        &self.app
    }
}
