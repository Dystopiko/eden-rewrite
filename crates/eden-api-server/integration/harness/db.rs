use super::TestHarness;

impl TestHarness {
    /// Acquires a single pooled connection from the primary database.
    ///
    /// Panics if the pool is exhausted or unavailable.
    pub async fn db_conn(&self) -> eden_postgres::PooledConnection {
        self.pools()
            .primary_db()
            .acquire()
            .await
            .expect("could not acquire db connection")
    }

    /// Acquires a write transaction from the primary database.
    ///
    /// Panics if the pool is exhausted or unavailable.
    pub async fn db_tx(&self) -> eden_postgres::Transaction<'_> {
        self.pools()
            .write()
            .await
            .expect("could not acquire db transaction")
    }
}
