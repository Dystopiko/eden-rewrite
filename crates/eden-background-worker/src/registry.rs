use erased_report::ErasedReport;
use std::{collections::HashMap, fmt, pin::Pin, sync::Arc, time::Duration};

use crate::job::BackgroundJob;

#[must_use]
#[derive(Clone)]
pub struct JobRegistry<C> {
    descriptors: HashMap<&'static str, JobDescriptor<C>>,
}

impl<C> JobRegistry<C>
where
    C: Clone + Send + Sync + 'static,
{
    /// Creates an empty [`Registry`].
    pub fn new() -> Self {
        Self {
            descriptors: HashMap::new(),
        }
    }

    pub fn register<J>(&mut self)
    where
        J: BackgroundJob<Context = C>,
    {
        self.descriptors.insert(J::TYPE, JobDescriptor::of::<J>());
    }

    #[must_use]
    pub fn contains<J: BackgroundJob<Context = C>>(&self) -> bool {
        self.descriptors.contains_key(J::TYPE)
    }

    #[must_use]
    pub fn get(&self, job_type: &str) -> Option<&JobDescriptor<C>> {
        self.descriptors.get(job_type)
    }

    #[must_use]
    pub fn types(&self) -> Vec<String> {
        self.descriptors.keys().map(|v| v.to_string()).collect()
    }
}

impl<C> Default for JobRegistry<C>
where
    C: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C> fmt::Debug for JobRegistry<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("registered_jobs", &self.descriptors.len())
            .finish()
    }
}

/// Everything the worker needs to execute and retry a particular job type.
#[derive(Clone)]
pub struct JobDescriptor<C> {
    /// How many times the job may be retried before it is permanently failed.
    /// `None` means retry indefinitely.
    pub max_retries: Option<u16>,
    /// Wall-clock limit for a single execution attempt.
    pub timeout: Duration,
    /// Type-erased entry point of the registered background job.
    pub execute: Arc<ExecuteFn<C>>,
}

impl<C> JobDescriptor<C>
where
    C: Clone + Send + Sync + 'static,
{
    fn of<J>() -> Self
    where
        J: BackgroundJob<Context = C>,
    {
        let execute: Arc<ExecuteFn<C>> = Arc::new(|ctx, data| {
            Box::pin(async move {
                let job: J = serde_json::from_value(data).map_err(ErasedReport::new)?;
                job.run(ctx).await
            })
        });

        Self {
            max_retries: J::MAX_RETRIES,
            timeout: J::TIMEOUT,
            execute,
        }
    }
}

type ExecuteFn<C> = dyn Fn(C, serde_json::Value) -> ExecuteFuture + Send + Sync;
type ExecuteFuture = Pin<Box<dyn Future<Output = Result<(), ErasedReport>> + Send>>;

#[cfg(test)]
#[allow(unused)]
mod tests {
    use crate::{
        job::BackgroundJob,
        registry::{JobDescriptor, JobRegistry},
    };
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, Serialize)]
    struct ExampleJob;

    impl BackgroundJob for ExampleJob {
        const TYPE: &'static str = "example";
        type Context = ();

        async fn run(&self, context: ()) -> Result<(), erased_report::ErasedReport> {
            Ok(())
        }
    }

    #[test]
    fn should_accept_any_unwind_safe_background_jobs() {
        let mut registry = JobRegistry::<()>::new();
        registry.register::<ExampleJob>();

        let item = registry.get(ExampleJob::TYPE);
        assert!(item.is_some());
    }

    fn requires_sync<T: Sync>() {}
    fn should_implement_sync() {
        requires_sync::<JobRegistry<()>>();
        requires_sync::<JobDescriptor<()>>();
    }
}
