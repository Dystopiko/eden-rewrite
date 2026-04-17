//! Configuration migration system.
//!
//! This module handles migrating configuration files between schema versions,
//! ensuring backward compatibility while allowing the configuration format to evolve.
//!
//! # Schema Versioning
//!
//! The schema version is stored in the `schema_version` field at the root level
//! of the configuration file. When not present, the version is inferred from
//! the configuration structure.
//!
//! # Migration Process
//!
//! Migrations are applied sequentially:
//! 1. Detect current schema version
//! 2. Apply all migrations from current to latest version
//! 3. Update `schema_version` field
use error_stack::{Report, ResultExt};
use thiserror::Error;
use toml_edit::DocumentMut;

mod v2;

#[derive(Debug)]
pub struct MigrationResult {
    pub warnings: Vec<&'static str>,
}

/// Errors that can occur during migration.
#[derive(Debug, Error)]
pub enum MigrationError {
    /// The schema version in the config is newer than what this code supports.
    #[error("unsupported schema version {found}, latest supported is {latest}")]
    UnsupportedVersion { found: u32, latest: u32 },

    /// Migration encountered an error.
    #[error("migration failed")]
    Failed,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord)]
pub enum SchemaVersion {
    /// Configuration schema version for Eden v2.0.0 to v2.3.x
    V2,

    /// The latest configuration schema version for Eden v2.4.0+
    #[default]
    V3,
}

impl SchemaVersion {
    /// The latest configuration schema version.
    pub const LATEST: Self = SchemaVersion::V3;
}

/// Migrates a configuration document to the latest schema version.
pub(crate) fn migrate(
    document: &mut DocumentMut,
) -> Result<MigrationResult, Report<MigrationError>> {
    let mut current_version = guess_schema_version(document);

    // already in the latest version
    let mut result = MigrationResult {
        warnings: Vec::new(),
    };

    if current_version == SchemaVersion::LATEST {
        return Ok(result);
    }

    // apply migrations sequentially
    while current_version < SchemaVersion::LATEST {
        match current_version {
            SchemaVersion::V2 => {
                self::v2::migrate_v2_to_v3(document, &mut result)
                    .change_context(MigrationError::Failed)
                    .attach("migrating from v2 to v3 failed")?;

                current_version = SchemaVersion::V3;
            }
            _ => unreachable!(),
        }
    }

    Ok(result)
}

/// Guesses the schema version based on the provided documentation.
#[allow(clippy::collapsible_if)]
#[must_use]
pub(crate) fn guess_schema_version(document: &toml_edit::Table) -> SchemaVersion {
    // Check for schema v2 indictators
    if let Some(sentry) = document.get("sentry").and_then(|v| v.as_table_like()) {
        if sentry.contains_key("env") {
            return SchemaVersion::V2;
        }
    }

    SchemaVersion::V3
}
