use async_channel::Sender;
use bon::Builder;
use eden_postgres::Pool;
use eden_signals::ShutdownSignal;
use erased_report::{EraseReportExt, ErasedReport};
use std::{collections::HashSet, time::Duration};
use tokio::time::MissedTickBehavior;

use crate::storage::{self, Row};

#[derive(Builder)]
pub struct JobDistributor {
    poll_interval: Duration,
    pool: Pool,
    worker_channels: Vec<(HashSet<String>, Sender<Row>)>,
    shutdown_signal: ShutdownSignal,

    #[builder(skip)]
    requeued_jobs: Vec<Row>,
}

impl JobDistributor {
    /// Runs the distribution loop until a shutdown signal is received.
    ///
    /// On each tick of the distributionr, it will:
    /// 1. Check for shutdown.
    /// 2. Wait for the next interval tick or an early shutdown signal.
    /// 3. Retry any requeued jobs.
    /// 4. Skip the database poll if every channel is full.
    /// 5. Pull a fresh batch of pending jobs and dispatch them.
    pub async fn run(&mut self) {
        tracing::debug!("job distributor started");

        let mut interval = tokio::time::interval(self.poll_interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            // Wait for the next tick, but wake early on shutdown so we don't
            // sit idle for a full interval after the signal arrives.
            self.tick().await;

            // Check for shutdown as`tick` is not cancel-safe, so we only check
            // at the bottom of the loop rather than inside a `select!`.
            if self.shutdown_signal.initiated() {
                tracing::debug!("job distributor terminated");
                break;
            }

            tokio::select! {
                _ = self.shutdown_signal.subscribe() => {},
                _ = interval.tick() => {},
            }
        }
    }

    /// Executes one distribution cycle: retry requeued jobs, then pull and
    /// dispatch fresh ones (unless all channels are full).
    async fn tick(&mut self) -> bool {
        self.retry_requeued_jobs();

        if self.all_channels_full() {
            return false;
        }

        match self.pull_incoming_jobs().await {
            Ok(jobs) => {
                for job in jobs {
                    self.dispatch(job);
                }
                true
            }
            Err(..) => false,
        }
    }

    /// Pulls the next batch of pending jobs from the database, marking them
    /// as running, and commits the transaction.
    async fn pull_incoming_jobs(&self) -> Result<Vec<Row>, ErasedReport> {
        let mut conn = self.pool.acquire().await?;
        storage::pull_next_pending(&mut conn, None, 50)
            .await
            .erase_report()
    }
}

impl JobDistributor {
    /// Attempts to dispatch `job` to the its designated non-full worker
    /// channel that accepts its type.
    ///
    /// Returns `true` if the job was sent. Returns `false` if no matching
    /// worker exists, the channel is closed, or the channel is full (the job
    /// is re-queued for the next cycle in that last case).
    fn dispatch(&mut self, job: Row) -> bool {
        // Resolve the index first so the shared borrow of `worker_channels`
        // ends before we potentially mutably borrow `requeued_jobs`.
        let Some(idx) = self
            .worker_channels
            .iter()
            .position(|(types, _)| types.contains(&job.job_type))
        else {
            tracing::warn!(
                job.type = ?job.job_type,
                "no worker channel registered for this job type",
            );
            return false;
        };

        match self.worker_channels[idx].1.try_send(job) {
            Ok(()) => true,
            Err(async_channel::TrySendError::Full(job)) => {
                self.requeued_jobs.push(job);
                false
            }
            Err(async_channel::TrySendError::Closed(..)) => false,
        }
    }

    /// Retries jobs that were previously held back because their target channel
    /// was full. Any job that still cannot be sent is kept in the queue.
    fn retry_requeued_jobs(&mut self) {
        // Drain into a temporary vec so we can call `dispatch` (which needs `&mut self`)
        // without holding a borrow on `requeued_jobs`.
        let pending = std::mem::take(&mut self.requeued_jobs);
        for job in pending {
            self.dispatch(job);
        }
    }

    /// Returns `true` if every worker channel is currently full, meaning
    /// there is no point in pulling new jobs from the database right now.
    fn all_channels_full(&self) -> bool {
        self.worker_channels.iter().all(|(_, tx)| tx.is_full())
    }
}

#[cfg(test)]
mod tests;
