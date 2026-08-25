//! Pull-consumer configuration and background-job execution.
//!
//! A [`JobWorker`] receives one message at a time from a shared durable
//! consumer, resolves it through [`JobRegistry`], executes it with timeout and
//! panic isolation, then explicitly acknowledges the outcome to JetStream.

use async_nats::jetstream::{
    AckKind, Message,
    consumer::{
        AckPolicy, PullConsumer,
        pull::{Config, Stream},
    },
};
use eden_signals::ShutdownSignal;
use erased_report::ErasedReport;
use error_stack::{Report, ResultExt};
use futures_util::{FutureExt, StreamExt};
use std::{collections::BTreeSet, panic::AssertUnwindSafe, time::Duration};
use thiserror::Error;
use tokio::time::{MissedTickBehavior, interval, timeout};
use tracing::{Instrument, debug, error, info_span, warn};

use crate::{
    JobRegistry,
    job::{JobDescriptor, JobFuture},
    queue::{JOB_SUBJECT_PREFIX, JOBS_SUBJECT, queue_subject},
};

/// Builds a durable pull consumer that receives every Eden job queue.
///
/// The consumer uses explicit acknowledgements. Do not combine this
/// all-queues filter with queue-specific consumers on the same work-queue
/// stream because their filters overlap.
#[must_use]
pub fn consumer_config(durable_name: impl Into<String>) -> Config {
    Config {
        durable_name: Some(durable_name.into()),
        ack_policy: AckPolicy::Explicit,
        filter_subject: JOBS_SUBJECT.into(),
        ..Default::default()
    }
}

/// Configures a durable consumer that only receives jobs from `queues`.
///
/// Queue names are deduplicated and sorted to produce deterministic
/// configuration. One queue uses JetStream's singular filter field; multiple
/// queues use its multi-filter field.
///
/// Filters belonging to different consumers on one work-queue stream must not
/// overlap.
///
/// # Panics
///
/// Panics if no queues are provided or a queue is not a single valid NATS
/// subject token.
#[must_use]
pub fn consumer_config_for<I, S>(durable_name: impl Into<String>, queues: I) -> Config
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let subjects = queues
        .into_iter()
        .map(|queue| queue_subject(queue.as_ref()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    assert!(!subjects.is_empty(), "at least one job queue is required");

    let mut config = Config {
        durable_name: Some(durable_name.into()),
        ack_policy: AckPolicy::Explicit,
        ..Default::default()
    };

    if subjects.len() == 1 {
        config.filter_subject = subjects[0].clone();
    } else {
        config.filter_subjects = subjects;
    }

    config
}

/// Executes registered jobs from one JetStream pull consumer.
///
/// A worker handles jobs sequentially. Create multiple workers with clones of
/// the same durable consumer to add concurrency; JetStream distributes
/// deliveries between them.
///
/// Shutdown stops message intake, but a job already being handled runs through
/// its normal acknowledgement path before the worker exits.
#[must_use = "workers do not process jobs until they are run"]
pub struct JobWorker<C> {
    context: C,
    consumer: PullConsumer,
    messages: Stream,
    registry: JobRegistry<C>,
    shutdown: ShutdownSignal,
}

/// Current server-side delivery counts for a job consumer.
///
/// Counts cover the subjects selected by that consumer, which may represent
/// one, several, or every logical job queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JobQueueStatus {
    /// Jobs that have not yet been delivered to a worker.
    pub pending: u64,

    /// Jobs delivered to workers but not yet acknowledged.
    pub in_flight: usize,
}

impl JobQueueStatus {
    /// Returns the number of pending and in-flight jobs.
    #[must_use]
    pub fn remaining(self) -> u64 {
        self.pending
            .saturating_add(u64::try_from(self.in_flight).unwrap_or(u64::MAX))
    }

    /// Returns whether there are no pending or unacknowledged jobs.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.remaining() == 0
    }
}

/// Reads the current delivery counts for the queues selected by `consumer`.
///
/// This performs a server request and returns a snapshot. A concurrent
/// publisher or worker can change the counts immediately afterward.
pub async fn job_queue_status(
    consumer: &PullConsumer,
) -> Result<JobQueueStatus, Report<WorkerError>> {
    let info = consumer
        .get_info()
        .await
        .map_err(|error| Report::new(WorkerError::InspectConsumer).attach(error.to_string()))?;

    Ok(JobQueueStatus {
        pending: info.num_pending,
        in_flight: info.num_ack_pending,
    })
}

impl<C> JobWorker<C>
where
    C: Clone + Send + Sync + 'static,
{
    /// Creates a worker and opens its pull-message stream.
    ///
    /// The registry must contain every job type matched by the consumer. An
    /// unregistered or malformed message is terminated rather than retried.
    pub async fn new(
        context: C,
        consumer: PullConsumer,
        registry: JobRegistry<C>,
        shutdown: ShutdownSignal,
    ) -> Result<Self, Report<WorkerError>> {
        let messages = consumer
            .messages()
            .await
            .change_context(WorkerError::Subscribe)?;

        Ok(Self {
            context,
            consumer,
            messages,
            registry,
            shutdown,
        })
    }

    /// Processes jobs until shutdown or a JetStream failure.
    pub async fn run(mut self) -> Result<(), Report<WorkerError>> {
        while let Some(message) = self.next_message().await? {
            self.handle(message).await?;
        }

        Ok(())
    }

    /// Processes jobs until the consumer's selected queues are empty.
    ///
    /// This is intended for tests after all jobs for the scenario have been
    /// enqueued. Concurrent publishers can add a job immediately after the
    /// final status check.
    pub async fn run_until_empty(mut self) -> Result<(), Report<WorkerError>> {
        while !job_queue_status(&self.consumer).await?.is_empty() {
            let Some(message) = self.next_message().await? else {
                return Ok(());
            };

            self.handle(message).await?;
        }

        Ok(())
    }

    async fn next_message(&mut self) -> Result<Option<Message>, Report<WorkerError>> {
        let message = tokio::select! {
            message = self.messages.next() => message,
            _ = self.shutdown.wait() => {
                debug!("background worker received shutdown signal");
                return Ok(None);
            }
        };

        let Some(message) = message else {
            return Err(Report::new(WorkerError::StreamEnded));
        };

        message.change_context(WorkerError::Receive).map(Some)
    }

    async fn handle(&self, message: Message) -> Result<(), Report<WorkerError>> {
        let subject = message.subject.as_str();
        let Some(job_key) = subject.strip_prefix(JOB_SUBJECT_PREFIX) else {
            warn!(%subject, "terminating message outside the Eden jobs subject");
            return acknowledge(&message, AckKind::Term).await;
        };

        let Some(descriptor) = self.registry.get(job_key) else {
            warn!(%job_key, "terminating unregistered background job");
            return acknowledge(&message, AckKind::Term).await;
        };

        let (queue, job_type) = job_key.split_once('.').expect(
            "registered job keys contain a validated queue and type separated by one period",
        );

        let delivery = message
            .info()
            .map_err(|error| Report::new(WorkerError::Inspect).attach(error.to_string()))?
            .delivered;
        let span = info_span!("job.run", %queue, %job_type, delivery);

        self.handle_registered(message, descriptor, delivery)
            .instrument(span)
            .await
    }

    async fn handle_registered(
        &self,
        message: Message,
        descriptor: &JobDescriptor<C>,
        delivery: i64,
    ) -> Result<(), Report<WorkerError>> {
        let future = match descriptor.decode(self.context.clone(), message.payload.clone()) {
            Ok(future) => future,
            Err(error) => {
                warn!(?error, "terminating background job with invalid payload");
                return acknowledge(&message, AckKind::Term).await;
            }
        };

        match execute(&message, future, descriptor.timeout).await? {
            Ok(()) => {
                debug!("background job completed");
                acknowledge(&message, AckKind::Ack).await
            }
            Err(error) if retries_exhausted(delivery, descriptor.max_retries) => {
                error!(?error, "background job exhausted its retries");
                acknowledge(&message, AckKind::Term).await
            }
            Err(error) => {
                warn!(?error, "background job failed and will be retried");
                acknowledge(&message, AckKind::Nak(Some(descriptor.retry_delay))).await
            }
        }
    }
}

async fn execute(
    message: &Message,
    future: JobFuture,
    timeout_after: Duration,
) -> Result<Result<(), ErasedReport>, Report<WorkerError>> {
    const PROGRESS_INTERVAL: Duration = Duration::from_secs(10);

    let mut future = Box::pin(timeout(
        timeout_after,
        AssertUnwindSafe(future).catch_unwind(),
    ));

    let mut progress = interval(PROGRESS_INTERVAL);
    progress.set_missed_tick_behavior(MissedTickBehavior::Skip);
    progress.tick().await;

    loop {
        tokio::select! {
            result = &mut future => {
                return Ok(match result {
                    Ok(Ok(result)) => result,
                    Ok(Err(payload)) => Err(panic_report(payload)),
                    Err(_) => Err(ErasedReport::new_from(JobTimedOut)),
                });
            }
            _ = progress.tick() => acknowledge(message, AckKind::Progress).await?,
        };
    }
}

async fn acknowledge(message: &Message, kind: AckKind) -> Result<(), Report<WorkerError>> {
    message
        .double_ack_with(kind)
        .await
        .map_err(|error| Report::new(WorkerError::Acknowledge).attach(error.to_string()))
}

fn retries_exhausted(delivery: i64, max_retries: Option<u16>) -> bool {
    max_retries.is_some_and(|max_retries| delivery > i64::from(max_retries))
}

#[derive(Debug, Error)]
/// Infrastructure failure while receiving or acknowledging background jobs.
pub enum WorkerError {
    /// The worker could not open its pull-message stream.
    #[error("failed to subscribe to background jobs")]
    Subscribe,

    /// The pull-message stream returned an error.
    #[error("failed to receive a background job")]
    Receive,

    /// The pull-message stream ended unexpectedly.
    #[error("background job stream ended")]
    StreamEnded,

    /// JetStream metadata for a delivered message was invalid.
    #[error("failed to inspect background job metadata")]
    Inspect,

    /// Current consumer delivery counts could not be retrieved.
    #[error("failed to inspect background job consumer")]
    InspectConsumer,

    /// JetStream did not confirm a message acknowledgement.
    #[error("failed to acknowledge background job")]
    Acknowledge,
}

#[derive(Debug, Error)]
#[error("background job timed out")]
struct JobTimedOut;

#[derive(Debug, Error)]
#[error("background job panicked")]
struct JobPanicked;

fn panic_report(payload: Box<dyn std::any::Any + Send + 'static>) -> ErasedReport {
    let cause = payload
        .downcast_ref::<&'static str>()
        .map(ToString::to_string)
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "<unknown>".into());

    ErasedReport::new_from(JobPanicked).attach(format!("panic cause: {cause}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configures_durable_explicit_ack_consumer() {
        let config = consumer_config("eden-workers");
        assert_eq!(config.durable_name.as_deref(), Some("eden-workers"));
        assert_eq!(config.ack_policy, AckPolicy::Explicit);
        assert_eq!(config.filter_subject, JOBS_SUBJECT);
    }

    #[test]
    fn configures_consumer_for_one_queue() {
        let config = consumer_config_for("email-workers", ["email"]);

        assert_eq!(config.filter_subject, "eden.jobs.email.>");
        assert!(config.filter_subjects.is_empty());
    }

    #[test]
    fn configures_consumer_for_selected_queues() {
        let config = consumer_config_for("test-workers", ["slow", "fast", "fast"]);

        assert!(config.filter_subject.is_empty());
        assert_eq!(
            config.filter_subjects,
            ["eden.jobs.fast.>", "eden.jobs.slow.>"]
        );
    }

    #[test]
    #[should_panic(expected = "at least one job queue is required")]
    fn rejects_empty_queue_selection() {
        _ = consumer_config_for::<_, &str>("empty-workers", []);
    }

    #[test]
    fn retries_include_the_first_delivery() {
        assert!(!retries_exhausted(1, Some(1)));
        assert!(retries_exhausted(2, Some(1)));
        assert!(!retries_exhausted(i64::MAX, None));
    }

    #[test]
    fn combines_pending_and_in_flight_jobs() {
        let status = JobQueueStatus {
            pending: 3,
            in_flight: 2,
        };

        assert_eq!(status.remaining(), 5);
        assert!(!status.is_empty());
        assert!(
            JobQueueStatus {
                pending: 0,
                in_flight: 0
            }
            .is_empty()
        );
    }
}
