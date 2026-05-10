use claims::assert_some;
use eden_model::tables::background_job::NewBackgroundJob;
use eden_postgres::Pool;
use eden_signals::ShutdownSignal;
use eden_timestamp::Timestamp;
use serde_json::json;
use std::{collections::HashSet, str::FromStr, time::Duration};
use uuid::Uuid;

use crate::{distributor::JobDistributor, storage};

#[sqlx::test(migrations = "../../migrations")]
async fn test_dispatch_for_two_distinct_workers(pool: sqlx::PgPool) {
    eden_test_util::init_tracing_for_tests();

    let pool: Pool = pool.into();
    let mut conn = pool.acquire().await.unwrap();
    prepare_sample_jobs(&mut conn).await;

    let (tx1, rx1) = async_channel::bounded(2);
    let (tx2, rx2) = async_channel::bounded(2);

    let mut distributor = make_distributor(
        pool,
        vec![(job_types(["job1"]), tx1), (job_types(["job2"]), tx2)],
    );

    distributor.tick().await;

    assert!(rx1.is_full(), "worker 1 channel should be full");
    assert!(rx2.is_full(), "worker 2 channel should be full");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_dispatch_for_one_distinct_worker(pool: sqlx::PgPool) {
    eden_test_util::init_tracing_for_tests();
    let pool: Pool = pool.into();

    let mut conn = pool.acquire().await.unwrap();
    prepare_sample_jobs(&mut conn).await;

    let (tx, rx) = async_channel::bounded(4);

    let mut distributor = make_distributor(pool, vec![(job_types(["job1", "job2"]), tx)]);
    distributor.tick().await;

    assert!(rx.is_full(), "worker channel should be full");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_all_channels_are_full(pool: sqlx::PgPool) {
    eden_test_util::init_tracing_for_tests();
    let pool: Pool = pool.into();

    let mut conn = pool.acquire().await.unwrap();
    prepare_sample_jobs(&mut conn).await;

    let (tx1, _rx1) = async_channel::bounded(1);
    let (tx2, _rx2) = async_channel::bounded(1);

    let mut distributor = make_distributor(
        pool,
        vec![(job_types(["job1"]), tx1), (job_types(["job2"]), tx2)],
    );

    assert!(!distributor.all_channels_full());

    let jobs = storage::pull_next_pending(&mut conn, None, 2)
        .await
        .unwrap();

    for job in jobs {
        distributor.dispatch(job);
    }

    assert!(distributor.all_channels_full());
}

#[sqlx::test(migrations = "../../migrations")]
async fn should_requeue_job_if_channel_is_full(pool: sqlx::PgPool) {
    eden_test_util::init_tracing_for_tests();
    let pool: Pool = pool.into();

    let mut conn = pool.acquire().await.unwrap();
    prepare_sample_jobs(&mut conn).await;

    let (tx, rx) = async_channel::bounded(1);

    let mut distributor = make_distributor(pool, vec![(job_types(["job1", "job2"]), tx)]);
    let jobs = storage::pull_next_pending(&mut conn, None, 2)
        .await
        .unwrap();

    assert!(
        distributor.dispatch(jobs[0].clone()),
        "first dispatch should succeed"
    );
    assert!(
        !distributor.dispatch(jobs[1].clone()),
        "second dispatch should fail (channel full)"
    );
    assert_eq!(
        distributor.requeued_jobs.len(),
        1,
        "overflowed job should be requeued"
    );

    drop(rx);
}

#[sqlx::test(migrations = "../../migrations")]
async fn should_dispatch_one_job(pool: sqlx::PgPool) {
    eden_test_util::init_tracing_for_tests();

    let pool: Pool = pool.into();

    let mut conn = pool.acquire().await.unwrap();
    prepare_sample_jobs(&mut conn).await;

    let (tx, rx) = async_channel::bounded(1);

    let mut distributor = make_distributor(pool, vec![(job_types(["job1", "job2"]), tx)]);
    let job = assert_some!(distributor.pull_incoming_jobs().await.unwrap().pop());

    assert!(distributor.dispatch(job));
    assert_some!(
        rx.try_recv().ok(),
        "receiver should contain the dispatched job"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn should_pull_incoming_jobs_in_priority_order(pool: sqlx::PgPool) {
    eden_test_util::init_tracing_for_tests();
    let pool: Pool = pool.into();

    let mut conn = pool.acquire().await.unwrap();
    let ids = prepare_sample_jobs(&mut conn).await;

    let (tx, _) = async_channel::bounded(1);
    let distributor = make_distributor(pool, vec![(job_types(["job1", "job2"]), tx)]);

    let pulled: Vec<Uuid> = distributor
        .pull_incoming_jobs()
        .await
        .unwrap()
        .into_iter()
        .map(|j| j.id)
        .collect();

    assert_eq!(pulled, [ids[1], ids[0], ids[2], ids[3]]);
}

fn make_distributor(
    pool: Pool,
    worker_channels: Vec<(HashSet<String>, async_channel::Sender<crate::storage::Row>)>,
) -> JobDistributor {
    JobDistributor::builder()
        .poll_interval(Duration::from_secs(1))
        .pool(pool)
        .shutdown_signal(ShutdownSignal::new())
        .worker_channels(worker_channels)
        .build()
}

fn job_types(types: impl IntoIterator<Item = &'static str>) -> HashSet<String> {
    types.into_iter().map(String::from).collect()
}

async fn prepare_sample_jobs(conn: &mut eden_postgres::Connection) -> [Uuid; 4] {
    let jobs = [
        (
            "2024-01-01T00:00:00Z",
            "a6b4fa28-40e7-4a07-b03d-2e3173016865",
            "job1",
            10,
        ),
        (
            "2024-01-01T00:00:00Z",
            "03ced58f-7792-4ac1-b9bd-b0e97c906948",
            "job2",
            100,
        ),
        (
            "2024-01-01T01:00:00Z",
            "43ca3d78-d3fd-4a30-95d3-d0c1d50f27f0",
            "job1",
            10,
        ),
        (
            "2024-01-01T01:30:00Z",
            "c7fa0962-73b9-4c25-bbbd-b4bea4f14e3f",
            "job2",
            1,
        ),
    ];

    let mut ids = [Uuid::nil(); 4];
    for (i, (created_at, id, kind, priority)) in jobs.into_iter().enumerate() {
        ids[i] = NewBackgroundJob::builder()
            .id(Uuid::from_str(id).unwrap())
            .created_at(Timestamp::from_str(created_at).unwrap())
            .job_type(kind)
            .priority(priority)
            .data(json!({}))
            .unwrap()
            .build()
            .enqueue(conn)
            .await
            .unwrap();
    }

    ids
}
