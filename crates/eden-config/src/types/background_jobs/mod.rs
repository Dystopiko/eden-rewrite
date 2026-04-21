use doku::Document;
use eden_file_diagnostics::RenderedDiagnostic;
use serde::Deserialize;

use crate::{context::SourceContext, validation::Validate};

/// Configuration for background job processing.
///
/// Controls the worker pool that processes asynchronous tasks such as
/// scheduled jobs, deferred operations, and other background work. This
/// system allows Eden to handle time-consuming operations without blocking
/// the main request/response cycle.
///
/// By default, background jobs are enabled with a single worker thread.
/// You can increase this for better concurrency on multi-core systems.
#[derive(Clone, Debug, Deserialize, Document, Eq, PartialEq)]
#[serde(default)]
pub struct BackgroundJobs {
    /// Whether background job processing is enabled.
    ///
    /// When set to `false`, the worker pool will not start and no
    /// background tasks will be processed. This can be useful for
    /// maintenance mode or testing scenarios.
    pub enabled: bool,

    /// Number of worker threads in the background job pool.
    ///
    /// Determines how many jobs can be processed concurrently. Setting
    /// this higher allows more parallel task execution but increases
    /// resource usage. Must be at least 1.
    ///
    /// Defaults to 1.
    #[doku(example = "1")]
    pub workers: usize,
}

impl Validate for BackgroundJobs {
    fn validate(&self, ctx: &SourceContext<'_>) -> Result<(), RenderedDiagnostic> {
        if self.workers == 0 {
            ctx.field_diagnostic(
                &["background_jobs", "workers"],
                "You cannot use number of workers to zero",
            )
            .with_note("You may want to set background_jobs.enabled to false")
            .emit()?;
        }
        Ok(())
    }
}

impl Default for BackgroundJobs {
    fn default() -> Self {
        Self {
            enabled: true,
            workers: 1,
        }
    }
}
