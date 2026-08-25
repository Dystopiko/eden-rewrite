//! JetStream stream configuration and background-job publishing.
//!
//! [`JobQueue`] serializes jobs and publishes them to subjects derived from
//! [`BackgroundJob::QUEUE`] and [`BackgroundJob::TYPE`]. Logical queues share
//! one work-queue stream and are separated by subject filters.

use async_nats::jetstream::{
    Context,
    message::PublishMessage,
    publish::PublishAck,
    stream::{Config, RetentionPolicy},
};
use error_stack::{Report, ResultExt};
use thiserror::Error;

use crate::BackgroundJob;

/// Prefix shared by every Eden background-job subject.
pub const JOB_SUBJECT_PREFIX: &str = "eden.jobs.";

/// Wildcard subject containing every Eden background-job queue.
pub const JOBS_SUBJECT: &str = "eden.jobs.>";

/// Builds the shared JetStream configuration for background jobs.
///
/// Work-queue retention removes a message after its consumer acknowledges it.
/// The returned configuration intentionally leaves storage limits, replica
/// count, and the duplicate window at their async-nats defaults so the caller
/// can override them before creating the stream.
#[must_use]
pub fn stream_config(name: impl Into<String>) -> Config {
    Config {
        name: name.into(),
        subjects: vec![JOBS_SUBJECT.into()],
        retention: RetentionPolicy::WorkQueue,
        ..Default::default()
    }
}

/// Publishes serialized [`BackgroundJob`] values to NATS JetStream.
///
/// Cloning a queue is cheap because the underlying JetStream context is
/// cloneable shared client state. The queue does not execute jobs or track
/// their completion; consumers and runners handle those responsibilities.
#[derive(Clone)]
pub struct JobQueue {
    jetstream: Context,
}

impl JobQueue {
    /// Creates a publisher backed by an existing JetStream context.
    #[must_use]
    pub fn new(jetstream: Context) -> Self {
        Self { jetstream }
    }

    /// Serializes and publishes one job.
    ///
    /// A successful [`PublishAck`] means JetStream accepted the message, not
    /// that a worker completed it.
    ///
    /// # Panics
    ///
    /// Panics if the job's queue or type is not a single valid NATS subject
    /// token.
    pub async fn enqueue<J>(&self, job: &J) -> Result<PublishAck, Report<EnqueueJobError>>
    where
        J: BackgroundJob,
    {
        self.publish(job, None).await
    }

    /// Enqueues a job once within the stream's duplicate window.
    ///
    /// `operation_id` must identify the operation rather than the serialized
    /// payload. Queue and job type are included in the NATS message ID, so the
    /// same operation ID can be reused by unrelated job types.
    ///
    /// Deduplication is bounded by the stream's configured duplicate window;
    /// this method does not provide permanent idempotency.
    ///
    /// # Panics
    ///
    /// Panics if the job's queue or type is not a single valid NATS subject
    /// token.
    pub async fn enqueue_once<J>(
        &self,
        operation_id: &str,
        job: &J,
    ) -> Result<PublishAck, Report<EnqueueJobError>>
    where
        J: BackgroundJob,
    {
        self.publish(job, Some(operation_id)).await
    }

    async fn publish<J>(
        &self,
        job: &J,
        operation_id: Option<&str>,
    ) -> Result<PublishAck, Report<EnqueueJobError>>
    where
        J: BackgroundJob,
    {
        let payload = serde_json::to_vec(job).change_context(EnqueueJobError::Serialize)?;
        let mut message = PublishMessage::build().payload(payload.into());

        if let Some(operation_id) = operation_id {
            message = message.message_id(format!("{}:{}:{operation_id}", J::QUEUE, J::TYPE));
        }

        self.jetstream
            .send_publish(job_subject(J::QUEUE, J::TYPE), message)
            .await
            .change_context(EnqueueJobError::Publish)?
            .await
            .change_context(EnqueueJobError::Acknowledge)
    }
}

pub(crate) fn job_key(queue: &str, job_type: &str) -> String {
    assert_subject_token("job queue", queue);
    assert_subject_token("job type", job_type);
    format!("{queue}.{job_type}")
}

pub(crate) fn job_subject(queue: &str, job_type: &str) -> String {
    format!("{JOB_SUBJECT_PREFIX}{}", job_key(queue, job_type))
}

pub(crate) fn queue_subject(queue: &str) -> String {
    assert_subject_token("job queue", queue);
    format!("{JOB_SUBJECT_PREFIX}{queue}.>")
}

fn assert_subject_token(label: &str, token: &str) {
    let valid = !token.is_empty()
        && token
            .chars()
            .all(|character| !character.is_whitespace() && !matches!(character, '.' | '*' | '>'));
    assert!(valid, "{label} {token:?} is not a valid NATS subject token");
}

/// Failure while serializing, publishing, or confirming a background job.
#[derive(Debug, Error)]
pub enum EnqueueJobError {
    /// The job could not be serialized as JSON.
    #[error("failed to serialize background job")]
    Serialize,

    /// The message could not be sent to NATS.
    #[error("failed to publish background job")]
    Publish,

    /// JetStream did not confirm that it stored the message.
    #[error("NATS did not acknowledge the background job")]
    Acknowledge,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_job_subject() {
        assert_eq!(
            job_subject("notifications", "send_alert"),
            "eden.jobs.notifications.send_alert"
        );
        assert_eq!(queue_subject("notifications"), "eden.jobs.notifications.>");
    }

    #[test]
    fn configures_work_queue_stream() {
        let config = stream_config("EDEN_JOBS");
        assert_eq!(config.name, "EDEN_JOBS");
        assert_eq!(config.subjects, [JOBS_SUBJECT]);
        assert_eq!(config.retention, RetentionPolicy::WorkQueue);
    }

    #[test]
    #[should_panic(expected = "job queue \"email.priority\" is not a valid NATS subject token")]
    fn rejects_nested_queue_name() {
        _ = queue_subject("email.priority");
    }

    #[test]
    #[should_panic(expected = "job type \"send.*\" is not a valid NATS subject token")]
    fn rejects_wildcard_job_type() {
        _ = job_subject("email", "send.*");
    }
}
