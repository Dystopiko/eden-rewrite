use erased_report::ErasedReport;
use error_stack::Report;
use sqlx::{error::DatabaseError, sqlite::SqliteError};

/// A high-level classification of a SQLite error, used to drive error handling
/// logic without requiring callers to inspect raw SQLite error codes directly.
///
/// This enum abstracts away the complexity of SQLite error codes and provides
/// a cleaner interface for error handling in application code.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SqliteErrorType {
    /// A unique constraint violation occurred. Contains the error message from SQLite.
    ///
    /// Maps to SQLite error code `SQLITE_CONSTRAINT_UNIQUE` (2067).
    UniqueViolation(String),

    /// The connection to the database is unhealthy or unavailable.
    ///
    /// This can occur due to pool timeouts, pool closure, worker crashes,
    /// database corruption, I/O errors, or when the database file cannot be opened.
    ///
    /// Maps to various SQLite error codes including:
    /// - `SQLITE_IOERR` (10) and its extended codes - Disk I/O errors
    /// - `SQLITE_CORRUPT` (11) - Database is corrupted
    /// - `SQLITE_FULL` (13) - Database or disk is full
    /// - `SQLITE_CANTOPEN` (14) - Unable to open database file
    /// - `SQLITE_PROTOCOL` (15) - Database lock protocol error
    /// - `SQLITE_NOTADB` (26) - File is not a valid database
    UnhealthyConnection,

    /// The database is in read-only mode and a write operation was attempted.
    ///
    /// Maps to SQLite error codes `SQLITE_READONLY` (8) and
    /// `SQLITE_READONLY_ROLLBACK` (776).
    Readonly,

    /// The requested row was not found in the database.
    ///
    /// This is typically returned by queries that expect a single row
    /// but find none, such as `query_one` or similar operations.
    RowNotFound,

    /// An unknown or unclassified SQLite error occurred.
    ///
    /// This variant is used for errors that don't fall into any of the
    /// other categories.
    Unknown,
}

impl SqliteErrorType {
    #[must_use]
    pub(crate) fn from_sqlx(error: &sqlx::Error) -> Self {
        use sqlx::Error::*;
        match error {
            PoolTimedOut | PoolClosed | WorkerCrashed => Self::UnhealthyConnection,
            RowNotFound => Self::RowNotFound,
            Database(inner) => Self::from_sqlite_error(inner.downcast_ref::<SqliteError>()),
            _ => Self::Unknown,
        }
    }

    #[must_use]
    pub(crate) fn from_sqlite_error(error: &SqliteError) -> SqliteErrorType {
        // SQLite extended result codes.
        // See: https://sqlite.org/rescode.html
        const SQLITE_CONSTRAINT_PRIMARYKEY: &str = "1555";
        const SQLITE_CONSTRAINT_UNIQUE: &str = "2067";

        // Connection health errors
        const SQLITE_IOERR: &str = "10";
        const SQLITE_CORRUPT: &str = "11";
        const SQLITE_FULL: &str = "13";
        const SQLITE_CANTOPEN: &str = "14";
        const SQLITE_PROTOCOL: &str = "15";
        const SQLITE_NOTADB: &str = "26";

        // Read-only errors
        const SQLITE_READONLY: &str = "8";
        const SQLITE_READONLY_ROLLBACK: &str = "776";

        match error.code().as_deref() {
            // https://sqlite.org/rescode.html#constraint_unique
            // https://sqlite.org/rescode.html#constraint_primarykey
            Some(SQLITE_CONSTRAINT_UNIQUE | SQLITE_CONSTRAINT_PRIMARYKEY) => {
                SqliteErrorType::UniqueViolation(error.message().to_string())
            }

            // Connection health errors
            // https://sqlite.org/rescode.html#cantopen
            // https://sqlite.org/rescode.html#notadb
            // https://sqlite.org/rescode.html#corrupt
            // https://sqlite.org/rescode.html#protocol
            // https://sqlite.org/rescode.html#full
            Some(
                SQLITE_CANTOPEN | SQLITE_NOTADB | SQLITE_CORRUPT | SQLITE_PROTOCOL | SQLITE_FULL
                | SQLITE_IOERR,
            ) => SqliteErrorType::UnhealthyConnection,

            // https://sqlite.org/rescode.html#ioerr
            // IOERR family includes many extended codes (all start with "10")
            Some(code) if code.starts_with("10") => SqliteErrorType::UnhealthyConnection,

            // https://sqlite.org/rescode.html#readonly
            // https://sqlite.org/rescode.html#readonly_recovery
            Some(SQLITE_READONLY | SQLITE_READONLY_ROLLBACK) => SqliteErrorType::Readonly,

            _ => SqliteErrorType::Unknown,
        }
    }
}

/// Extension trait that classifies a [`Report`] into a [`SqliteErrorType`].
pub trait SqliteReportExt {
    /// Attempts to classify the error in this report as a SQLite error.
    ///
    /// Returns `Some(SqliteErrorType)` if the report contains a SQLite error,
    /// or `None` if it doesn't contain a recognizable SQLite error.
    fn sqlite_error_type(&self) -> Option<SqliteErrorType>;
}

impl<C> SqliteReportExt for Report<C> {
    fn sqlite_error_type(&self) -> Option<SqliteErrorType> {
        self.downcast_ref::<sqlx::Error>()
            .map(SqliteErrorType::from_sqlx)
    }
}

impl SqliteReportExt for ErasedReport {
    fn sqlite_error_type(&self) -> Option<SqliteErrorType> {
        self.downcast_ref::<sqlx::Error>()
            .map(SqliteErrorType::from_sqlx)
    }
}

/// Extension trait that classifies a [`std::result::Result`] into a [`SqliteErrorType`].
pub trait SqliteResultExt {
    /// Attempts to classify any error in this result as a SQLite error.
    ///
    /// Returns `Some(SqliteErrorType)` if the result is an `Err` containing
    /// a SQLite error, or `None` if the result is `Ok` or doesn't contain
    /// a recognizable SQLite error.
    fn sqlite_error_type(&self) -> Option<SqliteErrorType>;
}

impl<T, E> SqliteResultExt for Result<T, Report<E>> {
    fn sqlite_error_type(&self) -> Option<SqliteErrorType> {
        match self {
            Ok(..) => None,
            Err(error) => error.sqlite_error_type(),
        }
    }
}

impl<T> SqliteResultExt for Result<T, ErasedReport> {
    fn sqlite_error_type(&self) -> Option<SqliteErrorType> {
        match self {
            Ok(..) => None,
            Err(error) => error.sqlite_error_type(),
        }
    }
}

/// Extension trait for converting "row not found" errors into `Option` values.
///
/// This trait provides the `optional` method which converts database query results
/// that might return a "row not found" error into an `Option<T>` instead. This is
/// useful for queries where finding no results is a valid outcome rather than an error.
pub trait QueryResultExt: Sized {
    type Okay;
    type Error;

    /// Converts a "row not found" error into `Ok(None)`.
    ///
    /// If the result is `Ok(value)`, returns `Ok(Some(value))`.
    /// If the result is `Err` with a `RowNotFound` SQLite error, returns `Ok(None)`.
    /// Otherwise, returns the original error.
    fn optional(self) -> Result<Option<Self::Okay>, Self::Error>;
}

impl<T, E> QueryResultExt for Result<T, Report<E>> {
    type Okay = T;
    type Error = Report<E>;

    fn optional(self) -> Result<Option<Self::Okay>, Self::Error> {
        match self {
            Ok(okay) => Ok(Some(okay)),
            Err(..) if matches!(self.sqlite_error_type(), Some(SqliteErrorType::RowNotFound)) => {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

impl<T> QueryResultExt for Result<T, ErasedReport> {
    type Okay = T;
    type Error = ErasedReport;

    fn optional(self) -> Result<Option<Self::Okay>, Self::Error> {
        match self {
            Ok(okay) => Ok(Some(okay)),
            Err(..) if matches!(self.sqlite_error_type(), Some(SqliteErrorType::RowNotFound)) => {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}
