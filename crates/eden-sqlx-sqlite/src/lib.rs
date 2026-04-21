//! SQLite database integration using SQLx.
pub mod error;
pub mod pool;

pub use self::error::SqliteErrorType;
pub use self::pool::{Connection, Pool, PoolError, PooledConnection, Transaction};
