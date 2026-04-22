use erased_report::ErasedReport;
use error_stack::Report;
use sqlx::postgres::PgDatabaseError;

/// A high-level classification of a PostgreSQL error, used to drive error handling
/// logic without requiring callers to inspect raw PostgreSQL error codes directly.
///
/// This enum abstracts away the complexity of PostgreSQL error codes (SQLSTATE codes)
/// and provides a cleaner interface for error handling in application code.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PgErrorType {
    /// A unique constraint violation occurred. Contains the error message from PostgreSQL.
    ///
    /// Maps to PostgreSQL error code `23505` (unique_violation).
    UniqueViolation(String),

    /// A foreign key constraint violation occurred.
    ///
    /// Maps to PostgreSQL error code `23503` (foreign_key_violation).
    ForeignKeyViolation(String),

    /// The connection to the database is unhealthy or unavailable.
    ///
    /// This can occur due to pool timeouts, pool closure, worker crashes,
    /// network errors, authentication failures, or server crashes.
    ///
    /// Maps to various conditions including:
    /// - Pool errors (timeout, closed, worker crash)
    /// - Connection errors (network issues, auth failures)
    /// - PostgreSQL Class 08 errors (connection exceptions)
    /// - PostgreSQL Class 57 errors (operator intervention)
    /// - PostgreSQL Class 58 errors (system errors)
    UnhealthyConnection,

    /// The database is in read-only mode and a write operation was attempted.
    ///
    /// Maps to PostgreSQL error code `25006` (read_only_sql_transaction).
    Readonly,

    /// The requested row was not found in the database.
    ///
    /// This is typically returned by queries that expect a single row
    /// but find none, such as `query_one` or similar operations.
    RowNotFound,

    /// An unknown or unclassified PostgreSQL error occurred.
    ///
    /// This variant is used for errors that don't fall into any of the
    /// other categories.
    Unknown,
}

impl PgErrorType {
    #[must_use]
    pub(crate) fn from_sqlx(error: &sqlx::Error) -> Self {
        use sqlx::Error::*;
        match error {
            PoolTimedOut | PoolClosed | WorkerCrashed => Self::UnhealthyConnection,
            RowNotFound => Self::RowNotFound,
            Database(inner) => Self::from_postgres_error(inner.downcast_ref::<PgDatabaseError>()),
            _ => Self::Unknown,
        }
    }

    #[must_use]
    pub(crate) fn from_postgres_error(error: &PgDatabaseError) -> PgErrorType {
        // PostgreSQL SQLSTATE error codes
        // See: https://www.postgresql.org/docs/current/errcodes-appendix.html

        // Class 23 - Integrity Constraint Violation
        const UNIQUE_VIOLATION: &str = "23505";
        const FOREIGN_KEY_VIOLATION: &str = "23503";

        // Class 25 - Invalid Transaction State
        const READ_ONLY_TRANSACTION: &str = "25006";

        match error.code() {
            // Constraint violations
            UNIQUE_VIOLATION => PgErrorType::UniqueViolation(error.message().to_string()),
            FOREIGN_KEY_VIOLATION => PgErrorType::ForeignKeyViolation(error.message().to_string()),

            // Transaction state errors
            READ_ONLY_TRANSACTION => PgErrorType::Readonly,

            // Connection and system errors
            // Class 08 - Connection Exception
            code if code.starts_with("08") => PgErrorType::UnhealthyConnection,
            // Class 57 - Operator Intervention (e.g., admin shutdown)
            code if code.starts_with("57") => PgErrorType::UnhealthyConnection,
            // Class 58 - System Error (e.g., I/O error, out of memory)
            code if code.starts_with("58") => PgErrorType::UnhealthyConnection,

            _ => PgErrorType::Unknown,
        }
    }
}

/// Extension trait that classifies a [`Report`] into a [`PgErrorType`].
pub trait PgReportExt {
    /// Attempts to classify the error in this report as a PostgreSQL error.
    ///
    /// Returns `Some(PgErrorType)` if the report contains a PostgreSQL error,
    /// or `None` if it doesn't contain a recognizable PostgreSQL error.
    fn pg_error_type(&self) -> Option<PgErrorType>;
}

impl<C> PgReportExt for Report<C> {
    fn pg_error_type(&self) -> Option<PgErrorType> {
        self.downcast_ref::<sqlx::Error>()
            .map(PgErrorType::from_sqlx)
    }
}

impl PgReportExt for ErasedReport {
    fn pg_error_type(&self) -> Option<PgErrorType> {
        self.downcast_ref::<sqlx::Error>()
            .map(PgErrorType::from_sqlx)
    }
}

/// Extension trait that classifies a [`std::result::Result`] into a [`PgErrorType`].
pub trait PgResultExt {
    /// Attempts to classify any error in this result as a PostgreSQL error.
    ///
    /// Returns `Some(PgErrorType)` if the result is an `Err` containing
    /// a PostgreSQL error, or `None` if the result is `Ok` or doesn't contain
    /// a recognizable PostgreSQL error.
    fn pg_error_type(&self) -> Option<PgErrorType>;
}

impl<T, E> PgResultExt for Result<T, Report<E>> {
    fn pg_error_type(&self) -> Option<PgErrorType> {
        match self {
            Ok(..) => None,
            Err(error) => error.pg_error_type(),
        }
    }
}

impl<T> PgResultExt for Result<T, ErasedReport> {
    fn pg_error_type(&self) -> Option<PgErrorType> {
        match self {
            Ok(..) => None,
            Err(error) => error.pg_error_type(),
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
    /// If the result is `Err` with a `RowNotFound` PostgreSQL error, returns `Ok(None)`.
    /// Otherwise, returns the original error.
    fn optional(self) -> Result<Option<Self::Okay>, Self::Error>;
}

impl<T, E> QueryResultExt for Result<T, Report<E>> {
    type Okay = T;
    type Error = Report<E>;

    fn optional(self) -> Result<Option<Self::Okay>, Self::Error> {
        match self {
            Ok(okay) => Ok(Some(okay)),
            Err(..) if matches!(self.pg_error_type(), Some(PgErrorType::RowNotFound)) => Ok(None),
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
            Err(..) if matches!(self.pg_error_type(), Some(PgErrorType::RowNotFound)) => Ok(None),
            Err(error) => Err(error),
        }
    }
}
