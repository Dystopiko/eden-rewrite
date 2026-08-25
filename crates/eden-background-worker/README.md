# `eden-background-worker`

This crate runs at-least-once background jobs through NATS JetStream. A job belongs to a logical
queue, and workers can consume every queue or only an explicit selection. Queue selection keeps
unrelated jobs from running in focused tests.

Jobs are published to `eden.jobs.<queue>.<type>`. The default queue is `default`.

```rust
use eden_background_worker::BackgroundJob;
use erased_report::ErasedReport;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct SendEmail {
    recipient: String,
}

impl BackgroundJob for SendEmail {
    const TYPE: &str = "send_email";
    const QUEUE: &str = "email";
    type Context = AppContext;

    async fn run(&self, context: Self::Context) -> Result<(), ErasedReport> {
        context.send_email(&self.recipient).await
    }
}

# #[derive(Clone)]
# struct AppContext;
# impl AppContext {
#     async fn send_email(&self, _: &str) -> Result<(), ErasedReport> { Ok(()) }
# }
```

## Low-level setup

`Runner` handles this setup for normal use. To construct a worker manually, create one work-queue
stream and register every job that worker may receive:

```rust,no_run
use async_nats::jetstream;
use eden_background_worker::{JobRegistry, stream_config};
# use eden_background_worker::BackgroundJob;
# use erased_report::ErasedReport;
# use serde::{Deserialize, Serialize};
# #[derive(Deserialize, Serialize)] struct SendEmail;
# #[derive(Clone)]
# struct AppContext;
# impl BackgroundJob for SendEmail {
#     const TYPE: &str = "send_email";
#     const QUEUE: &str = "email";
#     type Context = AppContext;
#     async fn run(&self, _: AppContext) -> Result<(), ErasedReport> { Ok(()) }
# }
# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let client = async_nats::connect("nats://localhost:4222").await?;
let jetstream = jetstream::new(client);
let stream = jetstream.create_stream(stream_config("EDEN_JOBS")).await?;
let registry = JobRegistry::new().register::<SendEmail>();
# let _ = (stream, registry);
# Ok(()) }
```

Consumer configuration is internal to `Runner`. `JobWorker` remains available for low-level use
when the caller already owns a compatible JetStream pull consumer.

## Worker behavior

A `JobWorker` receives jobs sequentially from a pull consumer and resolves each `queue.type` subject
through its registry. Invalid payloads and unregistered job types are terminated because retrying
cannot make them valid. Successful jobs are acknowledged; failed jobs are negatively acknowledged
with their configured retry delay; jobs that exhaust retries are terminated.

Each attempt is isolated from panics and limited by the job's timeout. While a job is running, the
worker periodically sends progress acknowledgements so JetStream does not redeliver long-running
work merely because the normal acknowledgement deadline elapsed. Eden still provides at-least-once,
not exactly-once, execution.

`ShutdownSignal` stops the worker from accepting another job. An attempt already in progress is
allowed to finish and follow its normal acknowledgement path.

## Runner

`Runner` is the usual entry point for a worker service. It creates or opens the job stream, creates
one durable consumer, and starts the configured number of workers against that consumer.

Defaults:

- Stream: `EDEN_JOBS`
- Durable consumer: `eden-workers`
- Queues: all queues under `eden.jobs.>`
- Workers: one

```rust,no_run
# use async_nats::jetstream;
# use eden_background_worker::{BackgroundJob, Runner};
# use erased_report::ErasedReport;
# use serde::{Deserialize, Serialize};
# #[derive(Deserialize, Serialize)] struct SendEmail;
# #[derive(Clone)] struct AppContext;
# impl BackgroundJob for SendEmail {
#     const TYPE: &str = "send_email";
#     const QUEUE: &str = "email";
#     type Context = AppContext;
#     async fn run(&self, _: AppContext) -> Result<(), ErasedReport> { Ok(()) }
# }
# async fn example(context: AppContext, jetstream: jetstream::Context) -> Result<(), Box<dyn std::error::Error>> {
let handle = Runner::new(context, jetstream)
    .register::<SendEmail>()
    .workers(4)
    .start()
    .await?;

// After the application receives its shutdown signal:
handle.shutdown().await?;
# Ok(()) }
```

All queues are consumed by default. Add `.queues(["email", "notifications"])` to restrict the
runner. Workers in one runner share its durable consumer, so JetStream distributes jobs among them.
Keep the returned `RunHandle`: dropping it detaches the worker tasks instead of stopping them.

`start` is for a long-running service. It returns a handle whose `status` method reports pending and
in-flight jobs and whose `shutdown` method stops intake, waits for active jobs, and joins every
worker. `run_until_empty` is the finite test-oriented path.

## Enqueueing

`JobQueue` is a lightweight publishing handle around a JetStream context. Every logical queue uses
the same stream and is routed by subject:

```text
eden.jobs.<queue>.<type>
```

Queue and type names must each be one non-empty NATS subject token. Dots, `*`, `>`, and whitespace
are rejected because they would change or broaden routing.

`JobQueue::enqueue` always publishes. `JobQueue::enqueue_once` attaches a caller-provided operation
ID namespaced by queue and job type; JetStream deduplicates it only for the stream's configured
duplicate window. A successful publish acknowledgement means JetStream stored the job; it does not
mean a worker has completed it.

```rust,no_run
# use async_nats::jetstream;
# use eden_background_worker::{BackgroundJob, JobQueue};
# use erased_report::ErasedReport;
# use serde::{Deserialize, Serialize};
# #[derive(Deserialize, Serialize)] struct SendEmail;
# impl BackgroundJob for SendEmail {
#     const TYPE: &str = "send_email";
#     const QUEUE: &str = "email";
#     type Context = ();
#     async fn run(&self, _: ()) -> Result<(), ErasedReport> { Ok(()) }
# }
# async fn example(jetstream: jetstream::Context) -> Result<(), Box<dyn std::error::Error>> {
let queue = JobQueue::new(jetstream);
queue.enqueue(&SendEmail).await?;
queue.enqueue_once("welcome:user-42", &SendEmail).await?;
# Ok(()) }
```

## Testing one queue

Give test-specific jobs a queue name, enqueue all data required by the scenario, then select that
queue with `Runner::queues`. Jobs on other queues remain pending and cannot introduce unrelated
side effects. Use unique stream and durable names per test so parallel tests do not share state.
JetStream work-queue streams reject overlapping consumer filters, so avoid combining an all-queues
consumer with queue-specific consumers on the same test stream.

Call `Runner::run_until_empty` to start the pool, process the selected queues, and stop as soon as
their consumer reports no pending or unacknowledged jobs:

```rust,no_run
# use eden_background_worker::Runner;
# async fn example<C: Clone + Send + Sync + 'static>(runner: Runner<C>) -> Result<(), Box<dyn std::error::Error>> {
runner.run_until_empty().await?;
# Ok(()) }
```

For assertions or monitoring, `RunHandle::status` returns `pending`, `in_flight`, `remaining()`, and
`is_empty()`. These values are a server-side snapshot for the queues selected by that runner.
Enqueue all test jobs before calling `run_until_empty`; a concurrent producer can publish
immediately after the final empty check.

Use unique stream and consumer names for isolated NATS test servers. On a shared work-queue stream,
queue filters for different consumers must not overlap. In particular, do not create an all-queues
consumer alongside a queue-specific test consumer on the same stream.

Delivery is at least once: a process can finish external work and stop before acknowledging the
message. Job implementations must therefore be idempotent, preferably by enforcing an operation ID
or state transition in the same PostgreSQL transaction as their database changes.
