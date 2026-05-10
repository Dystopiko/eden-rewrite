use async_channel::Receiver;
use eden_futures::CatchUnwind;
use eden_postgres::Pool;
use eden_signals::ShutdownSignal;
use erased_report::ErasedReport;
use std::{panic::PanicHookInfo, sync::Arc, time::Duration};
use thiserror::Error;
use tracing::{Instrument, debug, trace};
use uuid::Uuid;

use crate::{
    registry::{JobDescriptor, JobRegistry},
    storage::{self, JobStatus, Row},
};

pub struct Worker<C> {
    pub(crate) context: C,
    pub(crate) pool: Pool,
    pub(crate) poll_interval: Duration,
    pub(crate) registry: Arc<JobRegistry<C>>,
    pub(crate) rx: Receiver<Row>,
    pub(crate) shutdown_signal: ShutdownSignal,
    pub(crate) shutdown_when_queue_empty: bool,
}

impl<C> Worker<C>
where
    C: Clone + Send + Sync + 'static,
{
    pub async fn run(&mut self) {
        loop {
            match self.run_next_job().await {
                Ok(Some(..)) => continue,
                Ok(None) if self.shutdown_when_queue_empty => {
                    debug!("no pending background worker jobs found; shutting down the worker...");
                    break;
                }
                Ok(None) => {
                    trace!("no pending background worker jobs found");
                }
                Err(error) => {
                    debug!(?error, "failed to run next job");
                }
            };

            // Wait for a background job to be completed before shutting this down
            if self.shutdown_signal.initiated() {
                break;
            }

            tokio::time::sleep(self.poll_interval).await;
        }
    }

    async fn run_next_job(&mut self) -> Result<Option<Uuid>, ErasedReport> {
        // Maybe the distributor has shut down or panicked. Worker loop
        // will have to wait for the shutdown and exit cleanly.
        let Ok(job) = self.rx.recv().await else {
            return Ok(None);
        };

        let span = tracing::info_span!(
            "worker.run_next_job",
            job.id = %job.id,
            job.created_at = ?job.created_at,
            job.type = ?job.job_type,
            job.last_retry = ?job.last_retry,
            job.priority = %job.priority,
            job.retries = %job.retries,
        );

        let Some(descriptor) = self.registry.get(&job.job_type) else {
            span.in_scope(|| tracing::warn!("unknown job type {:?}", job.job_type));
            return Ok(None);
        };

        if !span.is_disabled() {
            span.record(
                "job.max_retries",
                tracing::field::debug(descriptor.max_retries),
            );
            span.record("job.timeout", tracing::field::debug(descriptor.timeout));
        }

        span.in_scope(|| tracing::debug!("found background job; running..."));

        let future = (*descriptor.execute)(self.context.clone(), job.data).catch_unwind();
        let result = tokio::time::timeout(descriptor.timeout, future)
            .await
            .map_err(|_| ErasedReport::new(JobTimedOut))
            .and_then(|res| res.map_err(make_panic_report))
            .flatten();

        let mut conn = self.pool.begin().await?;
        self.handle_job_result(&mut conn, job.id, descriptor, result)
            .instrument(span)
            .await?;

        conn.commit().await.map_err(ErasedReport::new)?;
        Ok(Some(job.id))
    }

    async fn handle_job_result(
        &self,
        conn: &mut eden_postgres::Transaction<'static>,
        job_id: Uuid,
        descriptor: &JobDescriptor<C>,
        result: Result<(), ErasedReport>,
    ) -> Result<(), ErasedReport> {
        let Err(error) = result else {
            tracing::debug!("deleting successful job");
            storage::delete(conn, job_id).await?;
            return Ok(());
        };

        if error.contains::<JobTimedOut>() {
            tracing::warn!(?error, "job got timed out");
        } else {
            tracing::warn!(?error, "failed to run job");
        }

        let status = storage::requeue_or_fail(conn, job_id, descriptor.max_retries).await?;
        if status == JobStatus::Failed {
            tracing::warn!(?error, "max tries exceeded; aborting background job");
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
#[error("background job timed out")]
struct JobTimedOut;

#[derive(Debug, Error)]
#[error("background job panicked")]
struct JobPanicked;

fn make_panic_report(payload: Box<dyn std::any::Any + Send + 'static>) -> ErasedReport {
    let cause = payload
        .downcast_ref::<PanicHookInfo<'_>>()
        .map(ToString::to_string)
        .or_else(|| {
            let cause = payload.downcast_ref::<&'static str>();
            cause.map(ToString::to_string)
        })
        .or_else(|| payload.downcast_ref::<String>().map(String::to_string))
        .unwrap_or_else(|| "<unknown>".into());

    ErasedReport::new(JobPanicked).attach(format!("panic cause: {cause}"))
}

#[cfg(test)]
mod tests;
