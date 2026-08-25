//! Worker-pool orchestration built on a shared JetStream pull consumer.
//!
//! [`Runner`] owns configuration and job registration. [`RunHandle`] owns the
//! running tasks and provides queue status, test draining, and graceful
//! shutdown.
use async_nats::jetstream::{Context, consumer::PullConsumer};
use eden_signals::ShutdownSignal;
use error_stack::{Report, ResultExt};
use std::time::Duration;
use thiserror::Error;
use tokio::task::JoinHandle;
use tracing::{Instrument, debug};

use crate::{
    BackgroundJob, JobQueueStatus, JobRegistry, JobWorker, stream_config,
    worker::{WorkerError, consumer_config, consumer_config_for, job_queue_status},
};

/// JetStream stream name used unless [`Runner::stream_name`] overrides it.
pub const DEFAULT_STREAM_NAME: &str = "EDEN_JOBS";

/// Durable consumer name used unless [`Runner::consumer_name`] overrides it.
pub const DEFAULT_CONSUMER_NAME: &str = "eden-workers";

const DEFAULT_WORKERS: usize = 1;
const STATUS_INTERVAL: Duration = Duration::from_millis(10);

/// Configures and starts a pool of background workers sharing one durable
/// JetStream pull consumer.
///
/// The runner consumes every queue by default. Use [`Runner::queues`] to
/// restrict it. Each registered job must use `C` as its
/// [`BackgroundJob::Context`].
///
/// Starting a runner creates the configured stream if it does not exist and
/// creates or updates its durable consumer. It then spawns the configured
/// number of [`JobWorker`] tasks.
#[must_use = "runners do not process jobs until they are started"]
pub struct Runner<C> {
    consumer_name: String,
    context: C,
    jetstream: Context,
    queues: Option<Vec<String>>,
    registry: JobRegistry<C>,
    stream_name: String,
    workers: usize,
}

impl<C> Runner<C>
where
    C: Clone + Send + Sync + 'static,
{
    /// Creates an all-queues, single-worker runner with the default stream and
    /// consumer names.
    pub fn new(context: C, jetstream: Context) -> Self {
        Self {
            consumer_name: DEFAULT_CONSUMER_NAME.into(),
            context,
            jetstream,
            queues: None,
            registry: JobRegistry::new(),
            stream_name: DEFAULT_STREAM_NAME.into(),
            workers: DEFAULT_WORKERS,
        }
    }

    /// Sets the durable consumer name shared by this runner's workers.
    ///
    /// Use a stable name in production so delivery state survives restarts.
    /// Tests should use a unique name to avoid sharing delivery state.
    pub fn consumer_name(mut self, name: impl Into<String>) -> Self {
        self.consumer_name = name.into();
        self
    }

    /// Restricts every worker in this runner to the selected queues.
    ///
    /// Queue names are deduplicated when the consumer configuration is built.
    ///
    /// # Panics
    ///
    /// Panics when `queues` is empty.
    pub fn queues<I, S>(mut self, queues: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let queues = queues.into_iter().map(Into::into).collect::<Vec<_>>();
        assert!(!queues.is_empty(), "at least one job queue is required");

        self.queues = Some(queues);
        self
    }

    /// Registers a job type that this runner can deserialize and execute.
    ///
    /// Registration does not select the job's queue. Use [`Runner::queues`]
    /// when this runner should consume only part of the registry.
    ///
    /// # Panics
    ///
    /// Panics if the same queue and job type were already registered.
    pub fn register<J>(mut self) -> Self
    where
        J: BackgroundJob<Context = C>,
    {
        self.registry = self.registry.register::<J>();
        self
    }

    /// Sets the JetStream stream name used by this runner.
    ///
    /// Changing the name does not change the stream subject, which remains
    /// `eden.jobs.>`.
    pub fn stream_name(mut self, name: impl Into<String>) -> Self {
        self.stream_name = name.into();
        self
    }

    /// Sets the number of worker tasks sharing the durable consumer.
    ///
    /// A single worker executes jobs sequentially. Additional workers allow
    /// JetStream to distribute jobs concurrently.
    ///
    /// # Panics
    ///
    /// Panics when `workers` is zero.
    pub fn workers(mut self, workers: usize) -> Self {
        self.workers = worker_count(workers);
        self
    }

    /// Creates the stream and durable consumer, then starts the worker pool.
    ///
    /// The returned run handle must be retained and shut down explicitly.
    /// Dropping it detaches the spawned tasks.
    pub async fn start(self) -> Result<RunHandle, Report<RunnerError>> {
        let stream = self
            .jetstream
            .get_or_create_stream(stream_config(self.stream_name))
            .await
            .map_err(|error| Report::new(RunnerError::CreateStream).attach(error.to_string()))?;

        let config = match self.queues {
            Some(queues) => consumer_config_for(&self.consumer_name, queues),
            None => consumer_config(&self.consumer_name),
        };

        let shutdown = ShutdownSignal::new();
        let consumer = stream
            .create_consumer(config)
            .await
            .map_err(|error| Report::new(RunnerError::CreateConsumer).attach(error.to_string()))?;

        let mut workers = Vec::with_capacity(self.workers);
        for id in 0..self.workers {
            workers.push(
                JobWorker::new(
                    self.context.clone(),
                    consumer.clone(),
                    self.registry.clone(),
                    shutdown.clone(),
                )
                .await
                .change_context(RunnerError::CreateWorker)?,
            );
            debug!(worker.id = id, "created background worker");
        }

        let handles = workers
            .into_iter()
            .enumerate()
            .map(|(id, worker)| {
                let span = tracing::info_span!("background_worker", worker.id = id);
                tokio::spawn(worker.run().instrument(span))
            })
            .collect();

        Ok(RunHandle {
            consumer,
            handles,
            shutdown,
        })
    }

    /// Starts workers and returns after their selected queues are empty.
    ///
    /// This is a convenience for `start().await?.wait_until_empty().await` and
    /// is intended for tests. Enqueue all jobs before calling it and do not run
    /// concurrent publishers while testing for emptiness.
    pub async fn run_until_empty(self) -> Result<(), Report<RunnerError>> {
        self.start().await?.wait_until_empty().await
    }
}

/// Owns a running worker pool and its shared durable consumer.
///
/// Use [`RunHandle::shutdown`] for long-running services or
/// [`RunHandle::wait_until_empty`] for finite test workloads.
#[must_use = "dropping the handle detaches its running background workers"]
pub struct RunHandle {
    consumer: PullConsumer,
    handles: Vec<JoinHandle<Result<(), Report<WorkerError>>>>,
    shutdown: ShutdownSignal,
}

impl RunHandle {
    /// Reads the current pending and in-flight counts from JetStream.
    ///
    /// The result covers exactly the queues selected by this handle's durable
    /// consumer and is only a snapshot at the time of the request.
    pub async fn status(&self) -> Result<JobQueueStatus, Report<RunnerError>> {
        job_queue_status(&self.consumer)
            .await
            .change_context(RunnerError::InspectConsumer)
    }

    /// Waits until no selected jobs remain, then gracefully stops all workers.
    ///
    /// Both pending and delivered-but-unacknowledged jobs must reach zero.
    /// This is intended for tests without concurrent publishers; a producer
    /// can otherwise publish immediately after the final status check.
    pub async fn wait_until_empty(self) -> Result<(), Report<RunnerError>> {
        let mut interval = tokio::time::interval(STATUS_INTERVAL);

        loop {
            interval.tick().await;
            let status = match self.status().await {
                Ok(status) => status,
                Err(error) => {
                    _ = self.shutdown().await;
                    return Err(error);
                }
            };

            if status.is_empty() {
                return self.shutdown().await;
            }
        }
    }

    /// Stops accepting new jobs and waits for active jobs to complete.
    ///
    /// Every worker is joined even if another worker failed. The first worker
    /// or join failure is returned.
    pub async fn shutdown(self) -> Result<(), Report<RunnerError>> {
        self.shutdown.initiate();

        let mut failure = None;
        for handle in self.handles {
            let error = match handle.await {
                Ok(Ok(())) => continue,
                Ok(Err(error)) => error.change_context(RunnerError::RunWorker),
                Err(error) => Report::new(RunnerError::JoinWorker).attach(error.to_string()),
            };

            failure.get_or_insert(error);
        }

        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

fn worker_count(workers: usize) -> usize {
    assert!(workers > 0, "at least one background worker is required");
    workers
}

#[derive(Debug, Error)]
/// Failure while creating, monitoring, running, or stopping a runner.
pub enum RunnerError {
    /// The configured JetStream stream could not be opened or created.
    #[error("failed to create the background job stream")]
    CreateStream,

    /// The durable pull consumer could not be created or updated.
    #[error("failed to create the background job consumer")]
    CreateConsumer,

    /// A worker could not subscribe to the durable consumer.
    #[error("failed to create a background worker")]
    CreateWorker,

    /// JetStream consumer status could not be retrieved.
    #[error("failed to inspect the background job consumer")]
    InspectConsumer,

    /// A worker returned an execution or JetStream error.
    #[error("background worker failed")]
    RunWorker,

    /// A spawned worker task panicked or was cancelled.
    #[error("failed to join a background worker")]
    JoinWorker,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "at least one background worker is required")]
    fn rejects_empty_worker_pool() {
        _ = worker_count(0);
    }
}
