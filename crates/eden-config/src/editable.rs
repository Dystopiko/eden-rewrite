//! Editable configuration file management.
//!
//! This module provides [`EditableConfig`], which enables safe reading, editing,
//! and writing of configuration files while preserving TOML formatting and comments.
use eden_toml::parse_as_document;
use error_stack::{Report, ResultExt};
use std::path::{Path, PathBuf};
use thiserror::Error;
use toml_edit::{Document, DocumentMut};

use crate::{
    migrations::{MigrationError, SchemaVersion, guess_schema_version},
    root::Config,
    validation::{Validate, ValidationContext},
};

/// A handle to a configuration file supporting both reading and writing.
pub struct EditableConfig {
    /// Path to the config file, regardless if it exists or not.
    path: PathBuf,

    /// The raw, unparsed TOML document
    document: Document<String>,
}

impl EditableConfig {
    /// Creates a new `EditableConfig` with an empty TOML document.
    #[must_use]
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            document: Document::parse("".to_string()).expect("should parse empty TOML file"),
        }
    }

    /// Applies edits to the configuration and writes changes atomically to disk.
    pub fn edit(
        &mut self,
        callback: impl FnOnce(&mut DocumentMut),
    ) -> Result<(), Report<EditConfigError>> {
        // Clone document and apply edits
        let mut modified_document = self.document.clone().into_mut();
        callback(&mut modified_document);

        // Write changes atomically
        let content = modified_document.to_string();
        eden_paths::write_atomic(&self.path, &content).change_context(EditConfigError)?;

        self.reload().change_context(EditConfigError)
    }

    /// Performs schema migrations on the configuration document.
    ///
    /// It returns [`MigrationResult`] containing details about the migration process.
    pub fn perform_migrations(&mut self) -> Result<(), Report<MigrationError>> {
        // Migrations must be strict: there's no automatic rollback if something
        // goes wrong, so we must ensure correctness at every step.
        let original_version = self.schema_version();

        let mut document = self.document.clone().into_mut();
        crate::migrations::migrate(&mut document)?;

        // Serialize and re-parse to validate the migrated document
        let toml = document.to_string();
        let migrated_document = parse_as_document(&toml, &self.path).unwrap_or_else(|_| {
            panic!(
                "migration produced invalid TOML: {original_version:?} -> {:?}",
                SchemaVersion::LATEST
            )
        });

        // Verify migration reached the target version
        let final_version = guess_schema_version(&migrated_document);
        if final_version != SchemaVersion::LATEST {
            panic!(
                "migration incomplete: {original_version:?} -> {final_version:?}, expected {:?}",
                SchemaVersion::LATEST
            );
        }

        // Parse the config for a safety check
        Config::maybe_toml_file(&toml, &self.path).unwrap_or_else(|error| {
            panic!("migration produced invalid latest schema: {error:?}");
        });

        // Atomically write to disk and update internal state
        eden_paths::write_atomic(&self.path, toml).change_context(MigrationError::Failed)?;
        self.document = migrated_document;

        Ok(())
    }

    /// Reloads the editable configuration from disk.
    pub fn reload(&mut self) -> Result<(), Report<LoadConfigError>> {
        let source = eden_paths::read(&self.path).change_context(LoadConfigError)?;
        self.document = parse_as_document(&source, &self.path).change_context(LoadConfigError)?;

        Ok(())
    }

    /// Gets the guessed schema version of the current configuration document.
    #[must_use]
    pub fn schema_version(&self) -> SchemaVersion {
        crate::migrations::guess_schema_version(&self.document)
    }

    /// Saves the current document to disk without reloading.
    ///
    /// Writes the current TOML document to the file path atomically.
    /// Unlike [`edit`](Self::edit), this does not reload or revalidate.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use eden_config::EditableConfig;
    ///
    /// let config = EditableConfig::new("eden.toml");
    /// // Make direct changes to document
    /// // ...
    /// config.save()?;
    /// # Ok::<(), error_stack::Report<eden_config::SaveConfigError>>(())
    /// ```
    #[track_caller]
    pub fn save(&self) -> Result<(), Report<SaveConfigError>> {
        let content = self.document.to_string();
        eden_paths::write_atomic(&self.path, content).change_context(SaveConfigError)
    }
}

impl EditableConfig {
    /// Returns a reference to the raw TOML document.
    ///
    /// The document can be modified directly, but changes are not persisted
    /// until [`save`](Self::save) or [`edit`](Self::edit) is called.
    ///
    /// Note: [`DocumentMut`] does not preserve source span information.
    #[must_use]
    pub fn document(&self) -> &Document<String> {
        &self.document
    }
}

impl EditableConfig {
    /// Parses the editable configuration into a validated [`Config`] struct.
    pub fn parse(&self) -> Result<Config, Report<LoadConfigError>> {
        let context = ValidationContext {
            source: self.document.raw(),
            path: &self.path,
            document: &self.document,
        };

        let config = eden_toml::deserialize::<Config>(&self.document, &self.path)
            .change_context(LoadConfigError)?;

        config.validate(&context).change_context(LoadConfigError)?;
        Ok(config)
    }
}

impl std::fmt::Debug for EditableConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditableConfig")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

/// Error returned when loading configuration fails.
#[derive(Debug, Error)]
#[error("failed to load Eden configuration")]
pub struct LoadConfigError;

/// Error returned when editing configuration fails.
#[derive(Debug, Error)]
#[error("failed to edit Eden configuration")]
pub struct EditConfigError;

/// Error returned when saving configuration fails.
#[derive(Debug, Error)]
#[error("failed to save Eden configuration")]
pub struct SaveConfigError;
