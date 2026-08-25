use erased_report::ErasedReport;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::time::Duration;

mod registry;
pub use self::registry::JobRegistry;

pub(crate) use self::registry::{JobDescriptor, JobFuture};

/// Queue used by jobs that do not select one explicitly.
pub const DEFAULT_QUEUE: &str = "default";

/// Default number of retries after the first delivery.
pub const DEFAULT_MAX_RETRIES: u16 = 5;

/// Default execution timeout for one attempt.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_mins(10);

/// Default delay before a failed job is delivered again.
pub const DEFAULT_RETRY_DELAY: Duration = Duration::from_secs(30);

/// A background job that can be submitted to NATS JetStream and executed
/// by worker instances asynchronously.
///
/// Implementors define a unique [`TYPE`](BackgroundJob::TYPE) within a queue,
/// execution behavior, timeout, and retry policy.
///
/// # Example
///
/// ```rust
/// use eden_background_worker::BackgroundJob;
/// use erased_report::ErasedReport;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Deserialize, Serialize)]
/// struct CleanupSessions {
///     older_than_hours: u64,
/// }
///
/// impl BackgroundJob for CleanupSessions {
///     const TYPE: &str = "cleanup_sessions";
///     type Context = ();
///
///     async fn run(&self, _ctx: ()) -> Result<(), ErasedReport> {
///         // cleanup logic here
///         Ok(())
///     }
/// }
/// ```
pub trait BackgroundJob: DeserializeOwned + Serialize + Send + Sync + 'static {
    /// A name that uniquely identifies this job type within its queue.
    ///
    /// Maps to the NATS subject `eden.jobs.<QUEUE>.<TYPE>`.
    /// It must be one non-empty NATS subject token: dots, wildcards, and
    /// whitespace are not accepted.
    const TYPE: &'static str;

    /// The logical queue that processes this job.
    ///
    /// It must follow the same subject-token restrictions as
    /// [`BackgroundJob::TYPE`].
    const QUEUE: &'static str = DEFAULT_QUEUE;

    /// How long a single execution attempt may run before it is considered timed out.
    const TIMEOUT: Duration = DEFAULT_TIMEOUT;

    /// How many times a failing job can be retried before it is given up on.
    ///
    /// `None` disables the retry limit entirely.
    const MAX_RETRIES: Option<u16> = Some(DEFAULT_MAX_RETRIES);

    /// Delay before JetStream redelivers a failed attempt.
    const RETRY_DELAY: Duration = DEFAULT_RETRY_DELAY;

    /// Shared state passed into every execution of this job.
    type Context: Clone + Send + Sync + 'static;

    /// Contains the actual work this job performs.
    fn run(&self, context: Self::Context) -> impl Future<Output = Result<(), ErasedReport>> + Send;
}
