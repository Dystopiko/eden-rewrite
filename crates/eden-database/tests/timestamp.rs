use eden_database::testing::TestPool;
use eden_timestamp::Timestamp;
use sqlx::Row;

#[tokio::test]
async fn should_encode_correctly() {
    eden_test_util::init_tracing_for_tests();

    let pool = TestPool::empty().await;
    let now = Timestamp::now();

    // Test round-trip encoding/decoding
    let mut conn = pool.acquire().await.unwrap();
    let row = sqlx::query("SELECT $1::TIMESTAMPTZ")
        .bind(now)
        .fetch_one(&mut *conn)
        .await
        .unwrap();

    let result = row.try_get::<Timestamp, _>(0).unwrap();

    // PostgreSQL stores timestamps with microsecond precision,
    // so we compare the timestamps by converting both to strings
    // which normalizes the precision
    assert_eq!(
        now.to_string(),
        result.to_string(),
        "Round-trip timestamp should match (accounting for precision)"
    );

    // Test that when we query the timestamp back as text, it has timezone info
    let row = sqlx::query("SELECT $1::TIMESTAMPTZ::TEXT")
        .bind(now)
        .fetch_one(&mut *pool.acquire().await.unwrap())
        .await
        .unwrap();

    let as_string = row.try_get::<String, _>(0).unwrap();
    assert!(
        as_string.contains("+00") || as_string.ends_with("Z") || as_string.contains("UTC"),
        "Timestamp string should include timezone information: {}",
        as_string
    );
}

#[tokio::test]
async fn should_decode_correctly() {
    eden_test_util::init_tracing_for_tests();

    let pool = TestPool::empty().await;

    // Test decoding from PostgreSQL's current_timestamp
    // Cast to TIMESTAMPTZ to ensure it's the right type
    let row = sqlx::query("SELECT (current_timestamp AT TIME ZONE 'UTC')::TIMESTAMPTZ")
        .fetch_one(&mut *pool.acquire().await.unwrap())
        .await
        .unwrap();

    // This should successfully decode as a Timestamp
    let timestamp = row.try_get::<Timestamp, _>(0).unwrap();

    // Verify it's a valid timestamp by checking elapsed time from Unix epoch
    let elapsed = timestamp.elapsed_from_unix();
    assert!(elapsed.is_some(), "Timestamp should be after Unix epoch");

    // Should be a reasonable time (after year 2000 but before year 2100)
    let elapsed_secs = elapsed.unwrap().as_secs();
    assert!(
        elapsed_secs > 946_684_800,
        "Timestamp should be after year 2000 (got {} seconds since epoch)",
        elapsed_secs
    );
    assert!(
        elapsed_secs < 4_102_444_800,
        "Timestamp should be before year 2100 (got {} seconds since epoch)",
        elapsed_secs
    );
}
