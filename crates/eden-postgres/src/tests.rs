//! Integration tests for the PostgreSQL connection pool.
//!
//! These tests verify that the pool works correctly with embedded PostgreSQL,
//! including test isolation, readonly mode, health checks, and transactions.

use claims::{assert_err, assert_ok};
use eden_config::types::database::{Common, DatabasePool};
use error_stack::ResultExt;
use std::time::Duration;

use crate::{
    error::{PgErrorType, PgResultExt},
    pool::Pool,
};

#[tokio::test]
async fn should_throw_unhealthy_connection_error() {
    let common = Common::builder()
        .connect_timeout(Duration::from_millis(10))
        .statement_timeout(Duration::from_millis(10))
        .build();

    let config = DatabasePool::builder()
        .url("postgres://127.0.0.2:11".to_string().into())
        .build();

    let pool = Pool::new(common, config).unwrap();
    let result = pool.acquire().await;
    assert_err!(&result);

    let kind = result.attach("").pg_error_type();
    assert_eq!(kind, Some(PgErrorType::UnhealthyConnection));
}

#[tokio::test]
async fn should_provide_testing_pool() {
    let pool = Pool::new_for_tests().build().await;

    let mut conn = pool.acquire().await.unwrap();
    let result = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&mut *conn)
        .await;

    assert_ok!(&result);
    assert_eq!(result.unwrap(), 1);
}

#[tokio::test]
async fn should_reject_writes_in_readonly_mode() {
    let pool = Pool::new_for_tests().readonly(true).build().await;

    let mut conn = pool.acquire().await.unwrap();
    let result = sqlx::query("CREATE TABLE numbers (number INTEGER)")
        .execute(&mut *conn)
        .await;

    assert_err!(&result);

    let kind = result.attach("").pg_error_type();
    assert_eq!(kind, Some(PgErrorType::Readonly));
}

#[tokio::test]
async fn should_isolate_test_databases() {
    let pool1 = Pool::new_for_tests().build().await;
    let pool2 = Pool::new_for_tests().build().await;

    sqlx::query("CREATE TABLE test_table (id INTEGER PRIMARY KEY)")
        .execute(pool1.inner())
        .await
        .unwrap();

    let result1 = sqlx::query("SELECT * FROM test_table")
        .fetch_all(pool1.inner())
        .await;

    assert_ok!(&result1);

    let result2 = sqlx::query("SELECT * FROM test_table")
        .fetch_all(pool2.inner())
        .await;

    assert_err!(&result2);
}

#[tokio::test]
async fn should_support_transactions() {
    let pool = Pool::new_for_tests().build().await;

    sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")
        .execute(pool.inner())
        .await
        .unwrap();

    let mut tx = pool.begin().await.unwrap();

    sqlx::query("INSERT INTO users (id, name) VALUES (1, 'Alice')")
        .execute(&mut *tx)
        .await
        .unwrap();

    sqlx::query("INSERT INTO users (id, name) VALUES (2, 'Bob')")
        .execute(&mut *tx)
        .await
        .unwrap();

    tx.commit().await.unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool.inner())
        .await
        .unwrap();

    assert_eq!(count, 2);
}

#[tokio::test]
async fn should_rollback_uncommitted_transactions() {
    let pool = Pool::new_for_tests().build().await;

    sqlx::query("CREATE TABLE items (id INTEGER PRIMARY KEY)")
        .execute(pool.inner())
        .await
        .unwrap();

    {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("INSERT INTO items (id) VALUES (1)")
            .execute(&mut *tx)
            .await
            .unwrap();
    }

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items")
        .fetch_one(pool.inner())
        .await
        .unwrap();

    assert_eq!(count, 0);
}

#[tokio::test]
async fn should_report_healthy_pool() {
    let pool = Pool::new_for_tests().build().await;

    let is_healthy = pool
        .check_health(Some(Duration::from_secs(1)))
        .await
        .unwrap();

    assert!(is_healthy);
}

#[tokio::test]
async fn should_handle_concurrent_test_pools() {
    let tasks: Vec<_> = (0..10)
        .map(|i| {
            tokio::spawn(async move {
                let pool = Pool::new_for_tests().build().await;

                let table_name = format!("table_{}", i);
                sqlx::query(&format!("CREATE TABLE {} (id INTEGER)", table_name))
                    .execute(pool.inner())
                    .await
                    .unwrap();

                let result = sqlx::query(&format!("SELECT * FROM {}", table_name))
                    .fetch_all(pool.inner())
                    .await;

                assert_ok!(result);
            })
        })
        .collect();

    for task in tasks {
        task.await.unwrap();
    }
}
