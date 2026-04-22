//! PostgreSQL database utilities for Eden.
//!
//! This crate provides a high-level interface for working with PostgreSQL databases,
//! including connection pooling, embedded PostgreSQL for testing, and error handling utilities.

pub mod embedded;
pub mod error;
pub mod pool;

pub use self::pool::{Connection, Pool, PoolError, PooledConnection, Transaction};

#[cfg(test)]
mod tests;
