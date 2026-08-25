use bytes::Bytes;
use erased_report::ErasedReport;
use serde_json::Error as DeserializeError;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use crate::{BackgroundJob, queue::job_key};

/// Maps job type names to their type-erased handlers.
///
/// Jobs are added through chained [`register`](JobRegistry::register) calls.
/// At runtime, the registry resolves incoming messages to the right handler.
///
/// # Example
///
/// ```rust,no_run
/// use eden_background_worker::JobRegistry;
/// # use eden_background_worker::BackgroundJob;
/// # use erased_report::ErasedReport;
/// # use serde::{Deserialize, Serialize};
/// # #[derive(Deserialize, Serialize)]
/// # struct JobA;
/// # impl BackgroundJob for JobA {
/// #     const TYPE: &str = "a";
/// #     type Context = ();
/// #     async fn run(&self, _: ()) -> Result<(), ErasedReport> { Ok(()) }
/// # }
/// # #[derive(Deserialize, Serialize)]
/// # struct JobB;
/// # impl BackgroundJob for JobB {
/// #     const TYPE: &str = "b";
/// #     type Context = ();
/// #     async fn run(&self, _: ()) -> Result<(), ErasedReport> { Ok(()) }
/// # }
///
/// let registry = JobRegistry::new()
///     .register::<JobA>()
///     .register::<JobB>();
/// ```
#[must_use]
#[derive(Clone)]
pub struct JobRegistry<C> {
    descriptors: HashMap<String, JobDescriptor<C>>,
}

impl<C> JobRegistry<C>
where
    C: Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            descriptors: HashMap::new(),
        }
    }

    /// Adds a [`BackgroundJob`] type so the worker knows how to handle it.
    ///
    /// Internally wraps the job in a type-erased closure that takes care of
    /// deserialization and dispatching to [`BackgroundJob::run`].
    pub fn register<J>(mut self) -> Self
    where
        J: BackgroundJob<Context = C>,
    {
        let key = job_key(J::QUEUE, J::TYPE);
        let previous = self
            .descriptors
            .insert(key.clone(), JobDescriptor::of::<J>());

        assert!(
            previous.is_none(),
            "background job {key:?} is already registered"
        );

        self
    }

    /// Checks whether a handler for job type `J` has been registered.
    #[must_use]
    pub fn contains<J: BackgroundJob<Context = C>>(&self) -> bool {
        self.descriptors.contains_key(&job_key(J::QUEUE, J::TYPE))
    }

    /// Lists every job type name that has been registered.
    pub fn types(&self) -> impl Iterator<Item = &str> + '_ {
        self.descriptors.keys().map(String::as_str)
    }

    /// Retrieves the [`JobDescriptor`] for the given job type name, if registered.
    #[must_use]
    pub(crate) fn get(&self, job_key: &str) -> Option<&JobDescriptor<C>> {
        self.descriptors.get(job_key)
    }
}

impl<C> Default for JobRegistry<C>
where
    C: Clone + Send + Sync + 'static,
{
    /// Creates a new empty [`JobRegistry`].
    fn default() -> Self {
        Self::new()
    }
}

/// Holds the execution closure and configuration for a single
/// registered job type.
#[derive(Clone)]
pub(crate) struct JobDescriptor<C> {
    decode: Arc<DecodeFn<C>>,

    pub max_retries: Option<u16>,
    pub retry_delay: Duration,
    pub timeout: Duration,
}

impl<C> JobDescriptor<C>
where
    C: Clone + Send + Sync + 'static,
{
    fn of<J>() -> Self
    where
        J: BackgroundJob<Context = C>,
    {
        let decode: Arc<DecodeFn<C>> = Arc::new(|ctx, payload| {
            let job: J = serde_json::from_slice(&payload)?;
            Ok(Box::pin(async move { job.run(ctx).await }))
        });

        Self {
            decode,
            max_retries: J::MAX_RETRIES,
            retry_delay: J::RETRY_DELAY,
            timeout: J::TIMEOUT,
        }
    }

    pub fn decode(&self, context: C, payload: Bytes) -> Result<JobFuture, DeserializeError> {
        (self.decode)(context, payload)
    }
}

pub(crate) type JobFuture = Pin<Box<dyn Future<Output = Result<(), ErasedReport>> + Send>>;

type DecodeFn<C> = dyn Fn(C, Bytes) -> Result<JobFuture, DeserializeError> + Send + Sync;

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Deserialize, Serialize)]
    struct Add(usize);

    impl BackgroundJob for Add {
        const TYPE: &str = "add";
        type Context = Arc<AtomicUsize>;

        async fn run(&self, context: Self::Context) -> Result<(), ErasedReport> {
            context.fetch_add(self.0, Ordering::Relaxed);
            Ok(())
        }
    }

    #[tokio::test]
    async fn decodes_and_runs_registered_job() {
        let context = Arc::new(AtomicUsize::new(0));
        let registry = JobRegistry::new().register::<Add>();
        let descriptor = registry.get("default.add").unwrap();
        let payload = serde_json::to_vec(&Add(3)).unwrap().into();

        descriptor
            .decode(context.clone(), payload)
            .unwrap()
            .await
            .unwrap();

        assert_eq!(context.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn rejects_invalid_payload() {
        let context = Arc::new(AtomicUsize::new(0));
        let registry = JobRegistry::new().register::<Add>();
        let descriptor = registry.get("default.add").unwrap();

        let result = descriptor.decode(context, Bytes::from_static(b"null"));
        assert!(result.is_err());
    }

    #[test]
    #[should_panic(expected = "background job \"default.add\" is already registered")]
    fn rejects_duplicate_job_type() {
        _ = JobRegistry::new().register::<Add>().register::<Add>();
    }

    #[derive(Deserialize, Serialize)]
    struct TestAdd(usize);

    impl BackgroundJob for TestAdd {
        const TYPE: &str = "add";
        const QUEUE: &str = "test";
        type Context = Arc<AtomicUsize>;

        async fn run(&self, context: Self::Context) -> Result<(), ErasedReport> {
            context.fetch_add(self.0, Ordering::Relaxed);
            Ok(())
        }
    }

    #[test]
    fn permits_the_same_type_in_different_queues() {
        let registry = JobRegistry::new().register::<Add>().register::<TestAdd>();
        assert!(registry.contains::<Add>());
        assert!(registry.contains::<TestAdd>());
        assert!(registry.get("default.add").is_some());
        assert!(registry.get("test.add").is_some());
    }
}
