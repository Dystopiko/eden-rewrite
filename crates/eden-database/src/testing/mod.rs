//! Test database utilities for creating isolated test environments.
//!
//! This module provides functionality for creating temporary PostgreSQL databases
//! for testing purposes. Each test database is created from a template that has
//! migrations pre-applied, allowing for fast test database creation.

// use eden_env_vars::required_var_parsed;
// use eden_postgres::Pool;
// use erased_report::{ErasedReport, IntoErasedReportExt};
// use rand::RngExt;
// use sqlx::migrate::MigrateError;
// use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
// use sqlx::{ConnectOptions, Connection, Executor};
// use std::fmt::Write;
// use std::time::{Duration, Instant};
// use tokio::sync::OnceCell;
// use url::Url;

#[cfg(test)]
pub(crate) mod krate;

// const TEST_SCHEMA_NAME: &str = "_eden_test";
// const TEMPLATE_DB_NAME: &str = "eden_database_template";
// const DB_NAME_SUFFIX_LENGTH: usize = 16;

// /// PostgreSQL advisory lock ID used to prevent race conditions during test database setup.
// const ADVISORY_LOCK_ID: i64 = 8318549251334697844;

// /// Global master pool used for database operations and test database creation.
// static MASTER_POOL: OnceCell<Pool> = OnceCell::const_new();

// /// Factory for creating isolated test database pools.
// ///
// /// [`TestPool`] provides methods to create temporary PostgreSQL databases for testing.
// /// Each database is isolated and can optionally include migrations. The databases
// /// are created from a template to improve performance.
// pub struct TestPool();

// impl TestPool {
//     /// Creates a new test database pool with migrations applied.
//     ///
//     /// This method creates a fresh database from a pre-migrated template,
//     /// making it fast and efficient for tests that need a fully set up database.
//     #[must_use]
//     pub async fn with_migrations() -> Pool {
//         Self::create_pool(true)
//             .await
//             .expect("failed to create test pool with migrations")
//     }

//     /// Creates a new empty test database pool without migrations.
//     ///
//     /// This method creates a fresh database without any schema or migrations applied,
//     /// useful for tests that need to verify migration logic or start from scratch.
//     #[must_use]
//     pub async fn empty() -> Pool {
//         Self::create_pool(false)
//             .await
//             .expect("failed to create empty test pool")
//     }

//     async fn create_pool(use_template: bool) -> Result<Pool, ErasedReport> {
//         ensure_template_database().await?;

//         let db_name = create_test_database(use_template).await?;
//         build_pool_for_database(&db_name).await
//     }
// }

// async fn ensure_template_database() -> Result<(), ErasedReport> {
//     MASTER_POOL
//         .get_or_init(|| async {
//             initialize_master_pool()
//                 .await
//                 .expect("failed to initialize master pool")
//         })
//         .await;

//     Ok(())
// }

// async fn check_template_database_exists(
//     conn: &mut eden_postgres::Connection,
// ) -> Result<bool, ErasedReport> {
//     sqlx::query_scalar::<_, bool>(
//         "SELECT EXISTS(
//             SELECT 1
//             FROM pg_database
//             WHERE datname = $1
//         )",
//     )
//     .bind(TEMPLATE_DB_NAME)
//     .fetch_one(&mut *conn)
//     .await
//     .erase_report()
// }

// async fn drop_template_database(conn: &mut eden_postgres::Connection) -> Result<(), ErasedReport> {
//     let cmd = format!("DROP DATABASE IF EXISTS {TEMPLATE_DB_NAME} WITH (FORCE)");
//     conn.execute(&*cmd).await.erase_report()?;

//     Ok(())
// }

// async fn create_template_database(
//     conn: &mut eden_postgres::Connection,
// ) -> Result<(), ErasedReport> {
//     let cmd = format!("CREATE DATABASE {TEMPLATE_DB_NAME}");
//     conn.execute(&*cmd).await.erase_report()?;

//     Ok(())
// }

// async fn apply_migrations_to_template(base_url: &Url) -> Result<bool, ErasedReport> {
//     let mut template_url = base_url.clone();
//     template_url.set_path(&format!("/{TEMPLATE_DB_NAME}"));

//     let now = Instant::now();
//     tracing::debug!("applying migrations for database template");

//     let mut conn = eden_postgres::Connection::connect(template_url.as_str())
//         .await
//         .expect("failed to connect to template database");

//     let result = crate::migrations::MIGRATIONS.run_direct(&mut conn).await;
//     if let Err(error) = result {
//         match error {
//             MigrateError::VersionMismatch(..) => return Ok(false),
//             _ => panic!("failed to run migrations on template database: {error:?}"),
//         }
//     }

//     let elapsed = now.elapsed();
//     tracing::debug!(
//         ?elapsed,
//         "successfully applied migrations for database template"
//     );

//     Ok(true)
// }

// async fn create_test_database(use_template: bool) -> Result<String, ErasedReport> {
//     let pool = MASTER_POOL
//         .get()
//         .expect("master pool should be initialized");

//     let mut conn = pool.acquire().await?;
//     let db_name = generate_test_database_name();

//     tracing::debug!(db.name = %db_name, ?use_template, "creating test database");
//     register_test_database(&mut conn, &db_name).await?;
//     execute_create_database(&mut conn, &db_name, use_template).await?;

//     Ok(db_name)
// }

// /// Registers a test database in the tracking table.
// async fn register_test_database(
//     conn: &mut eden_postgres::Connection,
//     db_name: &str,
// ) -> Result<(), ErasedReport> {
//     let query = format!("INSERT INTO {TEST_SCHEMA_NAME}.databases(name) VALUES ($1)");
//     sqlx::query(&query)
//         .bind(db_name)
//         .execute(&mut *conn)
//         .await
//         .erase_report()?;

//     Ok(())
// }

// /// Executes the CREATE DATABASE statement.
// async fn execute_create_database(
//     conn: &mut eden_postgres::Connection,
//     db_name: &str,
//     use_template: bool,
// ) -> Result<(), ErasedReport> {
//     let query = if use_template {
//         format!("CREATE DATABASE {db_name} TEMPLATE {TEMPLATE_DB_NAME}")
//     } else {
//         format!("CREATE DATABASE {db_name}")
//     };

//     conn.execute(&*query).await.erase_report()?;
//     Ok(())
// }

// /// Builds a connection pool for the specified database.
// async fn build_pool_for_database(db_name: &str) -> Result<Pool, ErasedReport> {
//     let url: Url = required_var_parsed("EDEN_TEST_DB_URL")?;
//     let url = PgConnectOptions::from_url(&url)
//         .erase_report()?
//         .database(db_name);

//     let master_pool = MASTER_POOL
//         .get()
//         .expect("master pool should be initialized")
//         .inner()
//         .clone();

//     let pool = PgPoolOptions::new()
//         .min_connections(0)
//         .max_connections(5)
//         // Close connections as soon as possible if left in the left queue
//         .idle_timeout(Some(Duration::from_secs(1)))
//         .test_before_acquire(true)
//         .after_connect(move |conn, _metadata| {
//             Box::pin(eden_postgres::pool::setup_pg_connection(
//                 conn,
//                 false,
//                 Duration::from_secs(15),
//             ))
//         })
//         // From: https://github.com/launchbadge/sqlx/blob/99af6eb1bfdc0ab4fb4951b33e4e01c53582ac3f/sqlx-postgres/src/testing/mod.rs#L103
//         // Immediately close master connections. Tokio's I/O streams don't like hopping runtimes.
//         .after_release(|_conn, _| Box::pin(async move { Ok(false) }))
//         .parent(master_pool)
//         .connect_lazy_with(url);

//     Ok(pool.into())
// }

// /// Initializes the master database pool and performs cleanup.
// async fn initialize_master_pool() -> Result<Pool, ErasedReport> {
//     let base_url: Url = required_var_parsed("EDEN_TEST_DB_URL")?;

//     let url = PgConnectOptions::from_url(&base_url).erase_report()?;
//     let pool = PgPoolOptions::new()
//         .min_connections(0)
//         .max_connections(20)
//         .acquire_timeout(Duration::from_secs(5))
//         .test_before_acquire(true)
//         .after_connect(move |conn, _metadata| {
//             Box::pin(eden_postgres::pool::setup_pg_connection(
//                 conn,
//                 false,
//                 Duration::from_secs(5),
//             ))
//         })
//         // From: https://github.com/launchbadge/sqlx/blob/99af6eb1bfdc0ab4fb4951b33e4e01c53582ac3f/sqlx-postgres/src/testing/mod.rs#L103
//         // Immediately close master connections. Tokio's I/O streams don't like hopping runtimes.
//         .after_release(|_conn, _| Box::pin(async move { Ok(false) }))
//         .connect_lazy_with(url);

//     let pool: Pool = pool.into();

//     let mut conn = pool.acquire().await?;
//     cleanup_stale_databases(&mut conn).await?;

//     let template_exists = check_template_database_exists(&mut conn).await?;
//     if !template_exists {
//         create_template_database(&mut conn).await?;
//     }

//     loop {
//         let needs_drop = !apply_migrations_to_template(&base_url).await?;
//         if needs_drop {
//             tracing::warn!("migration version mismatch detected, recreating template database");
//             drop_template_database(&mut conn).await?;
//             create_template_database(&mut conn).await?;
//             continue;
//         }
//         break;
//     }

//     Ok(pool)
// }

// /// Cleans up stale test databases from previous test runs.
// ///
// /// This function:
// /// 1. Creates the test tracking schema and table if they don't exist
// /// 2. Attempts to drop all registered test databases
// /// 3. Removes successfully dropped databases from the tracking table
// async fn cleanup_stale_databases(conn: &mut eden_postgres::Connection) -> Result<(), ErasedReport> {
//     initialize_test_tracking_schema(conn).await?;

//     let stale_databases = fetch_stale_test_databases(conn).await?;
//     if stale_databases.is_empty() {
//         return Ok(());
//     }

//     let dropped_databases = drop_stale_databases(conn, &stale_databases).await?;
//     remove_dropped_databases_from_tracking(conn, &dropped_databases).await?;

//     Ok(())
// }

// /// Initializes the test tracking schema and table.
// async fn initialize_test_tracking_schema(
//     conn: &mut eden_postgres::Connection,
// ) -> Result<(), ErasedReport> {
//     let query = format!(
//         r#"
//         SELECT pg_advisory_xact_lock({ADVISORY_LOCK_ID});

//         CREATE SCHEMA IF NOT EXISTS {TEST_SCHEMA_NAME};

//         CREATE TABLE IF NOT EXISTS {TEST_SCHEMA_NAME}.databases (
//             name VARCHAR(255) PRIMARY KEY,
//             created_at TIMESTAMPTZ NOT NULL DEFAULT now()
//         );

//         CREATE INDEX IF NOT EXISTS databases_created_at
//             ON {TEST_SCHEMA_NAME}.databases(created_at);
//         "#
//     );

//     conn.execute(query.as_str()).await.erase_report()?;
//     Ok(())
// }

// /// Fetches all stale test databases from the tracking table.
// async fn fetch_stale_test_databases(
//     conn: &mut eden_postgres::Connection,
// ) -> Result<Vec<String>, ErasedReport> {
//     sqlx::query_scalar(&format!("SELECT name FROM {TEST_SCHEMA_NAME}.databases"))
//         .fetch_all(&mut *conn)
//         .await
//         .erase_report()
// }

// /// Attempts to drop stale test databases and returns successfully dropped names.
// async fn drop_stale_databases(
//     conn: &mut eden_postgres::Connection,
//     database_names: &[String],
// ) -> Result<Vec<String>, ErasedReport> {
//     let mut command = String::with_capacity(64);
//     let mut dropped = Vec::with_capacity(database_names.len());

//     for db_name in database_names {
//         command.clear();
//         write!(
//             &mut command,
//             "DROP DATABASE IF EXISTS {db_name} WITH (FORCE)"
//         )
//         .expect("writing to string cannot fail");

//         match conn.execute(&*command).await {
//             Ok(_) => {
//                 tracing::debug!(db.name = %db_name, "dropped stale test database");
//                 dropped.push(db_name.clone());
//             }
//             Err(sqlx::Error::Database(ref db_err)) => {
//                 tracing::warn!(
//                     db.name = %db_name,
//                     error = ?db_err,
//                     "failed to drop stale test database"
//                 );
//             }
//             Err(err) => return Err(err).erase_report(),
//         }
//     }

//     tracing::debug!(
//         "successfully dropped {} stale test database(s)",
//         dropped.len()
//     );

//     Ok(dropped)
// }

// /// Removes successfully dropped databases from the tracking table.
// async fn remove_dropped_databases_from_tracking(
//     conn: &mut eden_postgres::Connection,
//     dropped_names: &[String],
// ) -> Result<(), ErasedReport> {
//     if dropped_names.is_empty() {
//         return Ok(());
//     }

//     sqlx::query(&format!(
//         "DELETE FROM {TEST_SCHEMA_NAME}.databases WHERE name = ANY($1::text[])"
//     ))
//     .bind(dropped_names)
//     .execute(&mut *conn)
//     .await
//     .erase_report()?;

//     Ok(())
// }

// /// Generates a random test database name with the prefix "eden_test_".
// fn generate_test_database_name() -> String {
//     format!("eden_test_{}", generate_random_suffix())
// }

// /// Generates a random alphanumeric suffix for database names.
// ///
// /// The suffix is lowercase to ensure PostgreSQL compatibility.
// fn generate_random_suffix() -> String {
//     let mut rng = rand::rng();
//     std::iter::repeat_with(|| (rng.sample(rand::distr::Alphabetic) as char).to_ascii_lowercase())
//         .take(DB_NAME_SUFFIX_LENGTH)
//         .collect()
// }
