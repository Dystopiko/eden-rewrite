pub(crate) async fn setup() -> eden_sqlx_sqlite::Pool {
    eden_test_util::init_tracing_for_tests();

    let pool = eden_sqlx_sqlite::Pool::memory(None).unwrap();
    crate::migrations::perform(&pool)
        .await
        .expect("failed to perform database migrations");

    pool
}
