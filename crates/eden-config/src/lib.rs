//! Configuration management for Eden with validation and migrations.
//!
//! # Migration System
//!
//! The configuration schema can evolve over time. The migration system automatically
//! upgrades older configuration files to the latest schema version while preserving
//! user data and settings.
//!
//! See [`migrations`] for details on the migration process.

mod editable;
mod root;
mod validation;

pub mod migrations;
pub mod types;

pub use self::editable::{EditConfigError, EditableConfig, LoadConfigError, SaveConfigError};
pub use self::root::Config;
pub use self::types::{Organization, Sentry, Setup};
