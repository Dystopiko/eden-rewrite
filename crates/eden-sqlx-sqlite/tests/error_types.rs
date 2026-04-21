use eden_config::types::database::{DatabasePool, SqliteUrl};
use eden_sqlx_sqlite::{Pool, SqliteErrorType, error::SqliteResultExt};
use erased_report::IntoErasedReportExt;

#[tokio::test]
async fn should_throw_readonly_error() {
    let config = DatabasePool::builder()
        .readonly(true)
        .url(SqliteUrl::MEMORY)
        .build();

    let pool = Pool::memory(Some(config)).unwrap();

    // Create a sample table to simulate an actual data collection
    let mut conn = pool.acquire().await.unwrap();
    let error_type = sqlx::query("CREATE TABLE numbers(number INTEGER UNIQUE NOT NULL);")
        .execute(&mut *conn)
        .await
        .erase_report()
        .sqlite_error_type();

    assert_eq!(error_type, Some(SqliteErrorType::Readonly));
}

#[tokio::test]
async fn should_throw_unique_violation_error() {
    let pool = Pool::memory(None).unwrap();

    // Create a sample table to simulate an actual data collection
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query(r" CREATE TABLE numbers(number INTEGER UNIQUE NOT NULL); ")
        .execute(&mut *conn)
        .await
        .unwrap();

    sqlx::query("INSERT INTO numbers(number) VALUES (67)")
        .execute(&mut *conn)
        .await
        .unwrap();

    let error_type = sqlx::query("INSERT INTO numbers(number) VALUES (67)")
        .execute(&mut *conn)
        .await
        .erase_report()
        .sqlite_error_type();

    assert_eq!(
        error_type,
        Some(SqliteErrorType::UniqueViolation(
            "UNIQUE constraint failed: numbers.number".into()
        ))
    );
}

#[tokio::test]
async fn should_throw_row_not_found_error() {
    let pool = Pool::memory(None).unwrap();

    // Create a sample table to simulate an actual data collection
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query(r" CREATE TABLE numbers(number INTEGER NOT NULL); ")
        .execute(&mut *conn)
        .await
        .unwrap();

    let error_type = sqlx::query_scalar::<_, i64>("SELECT * FROM numbers WHERE number = 67")
        .fetch_one(&mut *conn)
        .await
        .erase_report()
        .sqlite_error_type();

    assert_eq!(error_type, Some(SqliteErrorType::RowNotFound));
}
