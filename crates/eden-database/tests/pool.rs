use claims::assert_err;
use eden_database::testing::TestPool;

#[tokio::test]
async fn should_create_empty_testing_pool() {
    eden_test_util::init_tracing_for_tests();

    let pool = TestPool::empty().await;

    let mut conn = pool.acquire().await.unwrap();
    let result = sqlx::query("INSERT INTO members(discord_user_id) VALUES (1234)")
        .execute(&mut *conn)
        .await;

    assert_err!(&result);

    let error = result.unwrap_err().to_string();
    assert!(error.contains(r#"relation "members" does not exist"#));
}
