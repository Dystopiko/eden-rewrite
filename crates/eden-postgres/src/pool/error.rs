use error_stack::Report;
use thiserror::Error;

use crate::error::PgErrorType;

/// Errors that can occur during pool operations.
///
/// These errors represent different classes of failures when interacting with
/// the connection pool, from connection acquisition to transaction management.
#[derive(Debug, Error)]
pub enum PoolError {
    /// A general pool operation failed.
    #[error("Pool operation failed")]
    General,

    /// The connection pool is unhealthy and cannot serve requests.
    #[error("Connection pool is unhealthy")]
    Unhealthy,
}

impl PoolError {
    /// Converts a `sqlx::Error` into a `PoolError` with appropriate classification.
    ///
    /// This function inspects the error type and determines whether it represents
    /// a connection health issue or a general failure.
    pub(super) fn from_sqlx(error: sqlx::Error) -> Report<Self> {
        let context = match PgErrorType::from_sqlx(&error) {
            PgErrorType::UnhealthyConnection => Self::Unhealthy,
            _ => Self::General,
        };
        Report::new(error).change_context(context)
    }
}

/// Error indicating an invalid PostgreSQL connection URL.
///
/// This error is returned during pool creation when the provided connection
/// URL cannot be parsed as a valid PostgreSQL connection string.
#[derive(Debug, Error)]
#[error("Invalid PostgreSQL connection URL")]
pub struct InvalidConnectionUrl;
