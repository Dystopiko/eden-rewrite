//! Editable configuration file management.
//!
//! This module provides [`EditableConfig`], which enables safe reading, editing,
//! and writing of configuration files while preserving TOML formatting and comments.
use eden_toml::parse_as_document;
use error_stack::{Report, ResultExt};
use std::path::{Path, PathBuf};
use thiserror::Error;
use toml_edit::{Document, DocumentMut};

use crate::{context::SourceContext, root::Config, validation::Validate};

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

    /// Reloads the editable configuration from disk.
    pub fn reload(&mut self) -> Result<(), Report<LoadConfigError>> {
        let source = eden_paths::read(&self.path).change_context(LoadConfigError)?;
        self.document = parse_as_document(&source, &self.path).change_context(LoadConfigError)?;

        Ok(())
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
    /// # Ok::<(), error_stack::Report<eden_config::editable::SaveConfigError>>(())
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
        let context = SourceContext {
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
