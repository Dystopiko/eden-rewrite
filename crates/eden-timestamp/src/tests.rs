use claims::{assert_err, assert_ok};
use eden_postgres::Pool;
use sqlx::Row;

use crate::Timestamp;

// Adopted from: https://github.com/twilight-rs/twilight/blob/5f6e4ae198fbd7a879e3eb5f58d133d0ee425b77/twilight-model/src/util/datetime/display.rs
#[test]
fn should_display_valid_rfc_3339() {
    const EXPECTED: &str = "2020-02-02T02:02:02.020Z";
    const TIME: i64 = 1_580_608_922_020_000;

    let timestamp = Timestamp::from_micros(TIME).expect("non zero");

    // Default formatter should be with microseconds.
    assert_eq!(EXPECTED, timestamp.to_string());
}

#[test]
fn should_parse_valid_rfc_3339_timestamp() {
    static VALID_CASES: &[&str] = &[
        "2026-03-02T21:06:33Z",
        "2026-03-02T21:06:33+08:00",
        "2026-03-02T13:06:33.123456-08:00",
        "1990-12-31T23:59:60Z", // Leap second
        "2026-03-02t21:06:33z", // Lowercase
        "2026-03-02 21:06:33Z", // Should accept this but not recommended
    ];

    for input in VALID_CASES {
        let result = Timestamp::parse(input);
        assert_ok!(
            result,
            "{input:?} is a valid RFC 3339 timestamp but it failed to parse"
        );
    }
}

#[test]
fn should_not_parse_invalid_rfc_3339_timestamp() {
    static INVALID_CASES: &[&str] = &[
        "2026-03-02T21:06:33",  // Missing Offset/Z
        "2026-02-30T21:06:33Z", // Non-existent date
        "2026-03-02T25:06:33Z", // Invalid hour
        "26-03-02T21:06:33Z",   // 2-digit year
    ];

    for input in INVALID_CASES {
        let result = Timestamp::parse(input);
        _ = assert_err!(
            result,
            "{input:?} is not a valid RFC 3339 timestamp but it was successfully parsed"
        );
    }
}

#[test]
fn should_not_parse_other_timestamp_formats() {
    static INVALID_CASES: &[&str] = &[
        "20260302T210633Z",                // ISO 8601 Basic
        "1772485593",                      // Unix Epoch
        "Mon, 02 Mar 2026 21:06:33 +0000", // RFC 2822
    ];

    for input in INVALID_CASES {
        let result = Timestamp::parse(input);
        _ = assert_err!(
            result,
            "{input:?} is not a valid RFC 3339 timestamp but it was successfully parsed"
        );
    }
}

#[tokio::test]
async fn should_encode_correctly() {
    eden_test_util::init_tracing_for_tests();

    let pool = Pool::new_for_tests().build().await;
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

    let pool = Pool::new_for_tests().build().await;

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
