//! Isolated PostgreSQL test database harness.
//!
//! This module provides utilities for spawning isolated PostgreSQL test schemas
//! and managing template schemas for fast, concurrent test execution.
//!
//! # Prerequisites
//!
//! Running tests using this harness requires PostgreSQL client utilities (specifically `pg_dump`)
//! to be installed and accessible in `PATH`.
//!
//! # Running Tests
//!
//! Tests using this harness require dynamically generated test database environment variables.
//! Instead of setting these manually, it should run in a command below:
//!
//! ```sh
//! cargo xtask test
//! ```
//!
//! This command above automatically calls [`prepare_template_db`], provisions the template schema,
//! generates a temporary DDL file, and injects the required environment variables into the
//! test runner.
//!
//! # How It Works
//! **This scheme is copied from**:
//! <https://github.com/rust-lang/crates.io/blob/837c386e9c79529707d1a7cbe063e01703cc3729/crates/crates_io_test_db/src/lib.rs>
//!
//! 1. **Template Initialization** (*before test runner starts*):
//!    - Calling [`prepare_template_db`] acquires a PostgreSQL advisory lock derived via CRC64 from
//!      `TEMPLATE_SCHEMA` (`eden_test_template`) to prevent race conditions across parallel test
//!      processes.
//!    - It cleans up any leftover test schemas matching `eden_test_[a-z0-9]{16}` from prior runs.
//!    - Pending Diesel migrations are applied to `eden_test_template`, and the resulting DDL
//!      definitions are written to a temporary file.
//!
//! 2. **Fast Schema Allocation**:
//!    - Calling [`TestDatabase::new`] generates a unique schema name (e.g. `eden_test_a1b2c3d4e5f6g7h8`).
//!    - It replaces `eden_test_template` references in the cached DDL string with the new schema
//!      name and batch-executes it on PostgreSQL, setting up all tables in milliseconds without
//!      re-running migrations.
//!    - The connection URL incorporates `options=--search_path=<schema>,public` so all Diesel
//!      queries automatically scope to the test schema.
//!
//! 3. **Automatic Cleanup**:
//!    - When a [`TestDatabase`] goes out of scope, its [`Drop`] implementation executes `DROP SCHEMA IF EXISTS "<schema>" CASCADE`, ensuring complete isolation between tests.

use std::path::PathBuf;
use std::process::Command;
use std::sync::LazyLock;
use std::{fs, io};

use crc_fast::CrcAlgorithm;
use diesel::connection::SimpleConnection;
use diesel::deserialize::QueryableByName;
use diesel::sql_types;
use diesel::{Connection, PgConnection, QueryResult, RunQueryDsl, sql_query};
use diesel_migrations::{FileBasedMigrations, MigrationHarness};
use eden_env_vars::{required_var_parsed, var_parsed};
use erased_report::{EraseReportExt, ErasedReport};
use error_stack::ResultExt;
use rand::RngExt;
use tempfile::NamedTempFile;
use thiserror::Error;
use tracing::{debug, instrument};
use url::Url;

/// Prefix for dynamically allocated PostgreSQL test schemas.
const TEST_SCHEMA_PREFIX: &str = "eden_test_";

/// Name of the PostgreSQL template schema used for replaying migrations.
const TEMPLATE_SCHEMA: &str = "eden_test_template";

/// An isolated PostgreSQL test database schema instance.
///
/// Automatically drops the allocated PostgreSQL schema when dropped.
#[derive(Debug)]
pub struct TestDatabase {
    schema: String,
    url: Url,
}

impl TestDatabase {
    /// Creates a new isolated test database schema populated with the template DDL.
    ///
    /// # Panics
    ///
    /// Panics if test environment variables provisioned by `cargo xtask test` are missing
    /// or if replaying the template DDL fails.
    #[instrument]
    pub fn new() -> Self {
        let management = TestContext::instance();
        let test_db = management.allocate();

        let ddl = management
            .template_ddl
            .replace(TEMPLATE_SCHEMA, &test_db.schema);

        let mut conn = management.get_connection();
        conn.batch_execute(&ddl)
            .expect("failed to replay template DDL into test schema");

        test_db
    }

    /// Creates a new empty test database schema without replaying template DDL.
    ///
    /// # Panics
    ///
    /// Panics if test environment variables provisioned by `cargo xtask test` are missing
    /// or if creating the schema fails.
    #[instrument]
    pub fn empty() -> Self {
        let management = TestContext::instance();
        let test_db = management.allocate();

        let mut conn = management.get_connection();
        sql_query(format!("CREATE SCHEMA \"{}\"", test_db.schema))
            .execute(&mut conn)
            .expect("failed to create empty test schema");

        test_db
    }

    /// Returns the automatically configured database connection URL string.
    #[must_use]
    pub fn url(&self) -> &str {
        self.url.as_str()
    }

    /// Returns the schema name allocated for this test database instance.
    #[must_use]
    pub fn schema(&self) -> &str {
        &self.schema
    }

    /// Alias for [`TestDatabase::schema`].
    #[must_use]
    pub fn db_name(&self) -> &str {
        &self.schema
    }

    /// Establishes a new Diesel [`PgConnection`] connected to this test database schema.
    ///
    /// # Panics
    ///
    /// This function will panic if connecting to the database fails.
    #[instrument(skip(self))]
    pub fn connect(&self) -> PgConnection {
        PgConnection::establish(self.url()).expect("failed to connect to the database")
    }
}

impl Default for TestDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        if let Ok(mut conn) = PgConnection::establish(TestContext::instance().base_url.as_str()) {
            debug!(schema = %self.schema, "dropping test schema");
            let _ = sql_query(format!("DROP SCHEMA IF EXISTS \"{}\" CASCADE", self.schema))
                .execute(&mut conn);
        }
    }
}

/// Prepares the template database schema by acquiring an advisory lock, running migrations,
/// cleaning up leftover test schemas, and writing out the template DDL.
///
/// # Errors
///
/// Returns an [`ErasedReport`] if database connection, lock acquisition,
/// migration, or temporary file creation fails.
pub fn prepare_template_db(url: &Url) -> Result<PathBuf, ErasedReport> {
    let mut conn = PgConnection::establish(url.as_str())
        .attach("failed to connect to the database")
        .erase_report()?;

    with_advisory_lock(&mut conn, TEMPLATE_SCHEMA, |conn| {
        cleanup_leftover_schemas(conn)?;

        sql_query(format!("CREATE SCHEMA IF NOT EXISTS \"{TEMPLATE_SCHEMA}\"")).execute(conn)?;
        sql_query(format!("SET search_path TO \"{TEMPLATE_SCHEMA}\", public")).execute(conn)?;

        let migrations = FileBasedMigrations::find_migrations_directory()?;
        conn.run_pending_migrations(migrations)
            .map_err(BoxedError::from)?;

        let mut tempfile = NamedTempFile::new()
            .attach("could not create temporary file for ddl")
            .erase_report()?;

        capture_database_to_ddl(url, &mut tempfile)?;

        let (_, path) = tempfile
            .keep()
            .attach("failed to persist temporary file")
            .erase_report()?;

        Ok(path)
    })
}

#[derive(Debug, Error)]
#[error("pg_dump did not finish successfully")]
struct PgDumpFailed;

/// Generates a template DDL based on the contents of the connected database.
///
/// You may read the documentation of the original source at:
/// <https://github.com/rust-lang/crates.io/blob/837c386e9c79529707d1a7cbe063e01703cc3729/crates/crates_io_test_db/src/lib.rs#L272-L286>
#[instrument(skip(out))]
fn capture_database_to_ddl(base_url: &Url, out: &mut dyn io::Write) -> Result<(), ErasedReport> {
    let pg_dump = match var_parsed::<PathBuf>("POSTGRES_BIN_DIR").erase_report()? {
        Some(dir) => dir.join("pg_dump"),
        None => PathBuf::from("pg_dump"),
    };

    debug!(pg_dump = %pg_dump.display(), "Capturing template schema DDL via pg_dump…");
    let output = Command::new(&pg_dump)
        .arg("--no-owner")
        .arg("--no-acl")
        .arg("--inserts")
        .arg(format!("--schema={TEMPLATE_SCHEMA}"))
        .arg(base_url.as_ref())
        .output()
        .attach("failed to run `pg_dump`")
        .erase_report()?;

    if !output.status.success() {
        let report = ErasedReport::new_from(PgDumpFailed)
            .attach(format!("exit code: {}", output.status))
            .attach(format!(
                "stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            ));

        return Err(report);
    }

    let raw = std::str::from_utf8(&output.stdout)
        .attach("pg_dump produced non-UTF-8 output")
        .erase_report()?;

    for line in raw.lines() {
        if line.starts_with('\\') {
            continue;
        }

        if line.trim_start().starts_with("SET transaction_timeout") {
            continue;
        }
        writeln!(out, "{line}")?;
    }

    writeln!(out, "\nRESET ALL;")?;
    Ok(())
}

/// Drops leftover test schemas matching the [`TEST_SCHEMA_PREFIX`] pattern.
fn cleanup_leftover_schemas(conn: &mut PgConnection) -> QueryResult<()> {
    #[derive(QueryableByName)]
    struct Schema {
        #[diesel(sql_type = sql_types::Text)]
        schema_name: String,
    }

    let leftovers: Vec<Schema> = sql_query(format!(
        "SELECT schema_name FROM information_schema.schemata \
        WHERE schema_name ~ '^{TEST_SCHEMA_PREFIX}[a-z0-9]{{16}}$'"
    ))
    .load(conn)?;

    for Schema { schema_name } in leftovers {
        debug!("dropping leftover test schema (name = {:?})", schema_name);
        let _ = sql_query(format!("DROP SCHEMA IF EXISTS \"{schema_name}\" CASCADE")).execute(conn);
    }

    Ok(())
}

/// Executes a closure within a PostgreSQL advisory lock derived from a key string.
fn with_advisory_lock<F, T>(
    conn: &mut PgConnection,
    key: &str,
    runner: F,
) -> Result<T, ErasedReport>
where
    F: FnOnce(&mut PgConnection) -> Result<T, ErasedReport>,
{
    debug!("acquiring PostgreSQL advisory lock (key={key:?})");

    let lock_key = compute_lock_key(key);
    sql_query("SELECT pg_advisory_lock($1)")
        .bind::<sql_types::BigInt, _>(lock_key)
        .execute(conn)
        .attach("failed to acquire advisory lock")
        .erase_report()?;

    let result = runner(conn);
    let _ = sql_query("SELECT pg_advisory_unlock($1)")
        .bind::<sql_types::BigInt, _>(lock_key)
        .execute(conn);

    result
}

fn compute_lock_key(name: &str) -> i64 {
    let mut hasher = crc_fast::Digest::new(CrcAlgorithm::Crc64GoIso);
    hasher.update(name.as_bytes());
    hasher.finalize() as i64
}

/// Singleton context managing process environment variable state and template DDL string.
struct TestContext {
    base_url: Url,
    template_ddl: String,
}

impl TestContext {
    fn instance() -> &'static Self {
        static INSTANCE: LazyLock<TestContext> = LazyLock::new(TestContext::new);
        &INSTANCE
    }

    fn new() -> Self {
        let base_url = required_var_parsed::<Url>("EDEN_TEST_DB_URL")
            .expect("could not get database url from process");

        let ddl_path = required_var_parsed::<PathBuf>("EDEN_TEST_DB_DDL_PATH")
            .expect("could not get ddl path");

        let template_ddl = fs::read_to_string(&ddl_path)
            .expect("failed to read template DDL file (EDEN_TEST_DB_DDL_PATH)");

        Self {
            base_url,
            template_ddl,
        }
    }

    fn allocate(&self) -> TestDatabase {
        let schema = format!("{TEST_SCHEMA_PREFIX}{}", generate_name());
        let url = url_with_search_path(&self.base_url, &schema);

        TestDatabase { schema, url }
    }

    fn get_connection(&self) -> PgConnection {
        PgConnection::establish(self.base_url.as_str()).expect("failed to connect to database")
    }
}

#[derive(Debug, Error)]
#[error("{0}")]
struct BoxedError(String);

impl From<Box<dyn std::error::Error + Send + Sync>> for BoxedError {
    fn from(value: Box<dyn std::error::Error + Send + Sync>) -> Self {
        BoxedError(value.to_string())
    }
}

/// Appends a PostgreSQL `options=--search_path={schema},public`
/// query parameter to a database URL.
fn url_with_search_path(base_url: &Url, schema: &str) -> Url {
    let mut url = base_url.clone();
    url.query_pairs_mut()
        .append_pair("options", &format!("--search_path={schema},public"));

    url
}

/// Generates a random 16-character lowercase alphanumeric
/// identifier for schema names.
fn generate_name() -> String {
    let mut rng = rand::rng();
    std::iter::repeat_n((), 16)
        .map(|_| rng.sample(rand::distr::Alphanumeric) as char)
        .collect::<String>()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(QueryableByName)]
    struct BoolRow {
        #[diesel(sql_type = diesel::sql_types::Bool)]
        v: bool,
    }

    #[derive(QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        c: i64,
    }

    #[test]
    fn creates_empty_database() {
        let db = TestDatabase::empty();
        let mut conn = db.connect();

        let row: BoolRow = sql_query("SELECT true AS v").get_result(&mut conn).unwrap();
        assert!(row.v);
    }

    #[test]
    fn creates_migrated_database_from_template() {
        let db = TestDatabase::new();
        let mut conn = db.connect();

        let row: BoolRow = sql_query("SELECT true AS v").get_result(&mut conn).unwrap();
        assert!(row.v);

        let count: CountRow = sql_query("SELECT COUNT(*) AS c FROM mc_link_challenges")
            .get_result(&mut conn)
            .unwrap();

        assert_eq!(count.c, 0);
    }
}
