use eden_postgres::Pool;
use eden_signals::ShutdownSignal;
use futures::future::join_all;
use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::Arc,
    time::Duration,
};
use tokio::task::JoinHandle;
use tracing::Instrument;

use crate::{
    distributor::JobDistributor, job::BackgroundJob, registry::JobRegistry, worker::Worker,
};

#[must_use = "runners do not do anything unless you run them"]
pub struct Runner<C> {
    context: C,
    distributor_poll_interval: Duration,
    pool: Pool,
    queues: HashMap<String, Queue<C>>,
    shutdown_when_queue_empty: bool,
}

impl<C> Runner<C>
where
    C: Clone + Send + Sync + 'static,
{
    pub fn new(context: C, pool: Pool) -> Self {
        Self {
            context,
            distributor_poll_interval: DEFAULT_POLL_INTERVAL,
            pool,
            queues: HashMap::new(),
            shutdown_when_queue_empty: false,
        }
    }

    pub fn configure_distributor_poll_interval(mut self, interval: Duration) -> Self {
        self.distributor_poll_interval = interval;
        self
    }

    pub fn configure_queue<F>(mut self, name: &str, f: F) -> Self
    where
        F: FnOnce(&mut Queue<C>) -> &Queue<C>,
    {
        f(self.queues.entry(name.into()).or_default());
        self
    }

    pub fn shutdown_when_queue_empty(mut self) -> Self {
        self.shutdown_when_queue_empty = true;
        self
    }

    pub fn start(&self) -> RunHandle {
        let shutdown_signal = ShutdownSignal::new();

        let queues = self.queues.len();
        let workers = self.queues.iter().fold(0usize, |i, (_, q)| i + q.workers);
        tracing::debug!("launching {queues} queue(s) with {workers} background job worker(s)");

        let mut handles = Vec::new();
        let mut worker_channels = Vec::new();

        for (name, queue) in self.queues.iter() {
            let registry = Arc::new(queue.registry.clone());
            let (tx, rx) = async_channel::bounded(queue.workers);

            for id in 0..queue.workers {
                let name = format!("background-worker-{name}-{id}");
                let span = tracing::info_span!("worker", worker.name = ?name);
                tracing::info!(worker.name = ?name, "starting worker");

                let mut worker = Worker {
                    context: self.context.clone(),
                    pool: self.pool.clone(),
                    poll_interval: queue.poll_interval,
                    registry: registry.clone(),
                    rx: rx.clone(),
                    shutdown_signal: shutdown_signal.clone(),
                    shutdown_when_queue_empty: self.shutdown_when_queue_empty,
                };

                let handle = tokio::spawn(async move { worker.run().instrument(span).await });
                handles.push(handle);
            }

            let mut test_types = HashSet::new();
            test_types.extend(registry.types());
            worker_channels.push((test_types, tx));
        }

        let mut distributor = JobDistributor::builder()
            .poll_interval(self.distributor_poll_interval)
            .pool(self.pool.clone())
            .shutdown_signal(shutdown_signal.clone())
            .worker_channels(worker_channels)
            .build();

        let handle = tokio::spawn(async move { distributor.run().await });
        handles.push(handle);

        RunHandle {
            handles,
            shutdown_signal,
        }
    }
}

#[must_use]
pub struct RunHandle {
    handles: Vec<JoinHandle<()>>,
    shutdown_signal: ShutdownSignal,
}

impl RunHandle {
    pub async fn shutdown(self) {
        tracing::info!(
            "shutting down {} background worker(s)",
            // Job distributor is also included, exclude it.
            self.handles.len().saturating_sub(1)
        );

        self.shutdown_signal.initiate();
        for result in join_all(self.handles).await {
            if let Err(error) = result {
                tracing::warn!(?error, "background worker task panicked");
            }
        }
    }
}

#[must_use = "queues do not do anything when used outside the Runner struct"]
pub struct Queue<C> {
    poll_interval: Duration,
    registry: JobRegistry<C>,
    workers: usize,
}

impl<C> fmt::Debug for Queue<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Queue")
            .field("poll_interval", &self.poll_interval)
            .field("registry", &self.registry)
            .field("workers", &self.workers)
            .finish()
    }
}

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(1);

impl<C> Default for Queue<C>
where
    C: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self {
            poll_interval: DEFAULT_POLL_INTERVAL,
            registry: JobRegistry::new(),
            workers: 1,
        }
    }
}

impl<C> Queue<C>
where
    C: Clone + Send + Sync + 'static,
{
    /// Sets the interval after which each worker polls for new jobs.
    pub fn poll_interval(&mut self, interval: Duration) -> &mut Self {
        self.poll_interval = interval;
        self
    }

    /// Register a new job type for this queue.
    pub fn register<J: BackgroundJob<Context = C>>(&mut self) -> &mut Self {
        self.registry.register::<J>();
        self
    }

    /// Sets the number of workers to spawn for a queue.
    pub fn workers(&mut self, workers: usize) -> &mut Self {
        self.workers = workers;
        self
    }
}
