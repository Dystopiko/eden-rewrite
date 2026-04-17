use eden_env_vars::var_parsed;
use eden_file_diagnostics::RenderedDiagnostic;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use toml_edit::Document;

use crate::{
    editable::EditableConfig,
    types::{Bot, Sentry, Setup},
    validation::{Validate, ValidationContext},
};

/// Root configuration for the Eden backend application.
///
/// This structure represents the top-level configuration read from `eden.toml`
/// and encompasses all configuration sections including bot settings, initial
/// setup configuration, and optional error tracking.
///
/// Use [`Config::find()`] to automatically search all locations.
#[derive(Clone, Debug, doku::Document, Deserialize, PartialEq, Validate)]
pub struct Config {
    /// Discord bot settings and authentication.
    ///
    /// Controls bot behavior, token configuration, and guild settings.
    pub bot: Bot,

    /// Default settings applied during initial setup.
    ///
    /// Defines the initial configuration values when Eden is first
    /// deployed or when resetting to defaults.
    #[serde(default)]
    pub setup: Setup,

    /// Optional error tracking and performance monitoring via Sentry.
    ///
    /// When configured, enables automated error reporting and performance
    /// tracing. Omit this section to disable Sentry integration entirely.
    pub sentry: Option<Sentry>,
}

impl Config {
    /// Creates a new [`EditableConfig`] handle for the given path.
    ///
    /// This is a convenience method equivalent to calling
    /// `EditableConfig::new(path)`. The editable config allows
    /// modifying the configuration file while preserving formatting.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use eden_config::Config;
    ///
    /// let mut editable = Config::editable("eden.toml");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn editable<P: AsRef<Path>>(path: P) -> EditableConfig {
        EditableConfig::new(path)
    }

    /// Generates a formatted TOML representation of [`Config`] with
    /// documentation for all fields and nested structures.
    #[must_use]
    pub fn template() -> String {
        use doku::toml::{AutoComments, DocComments, Formatting, Layout, Spacing, ValuesStyle};

        doku::to_toml_fmt::<Self>(&Formatting {
            auto_comments: AutoComments {
                array_size: false,
                optional: false,
            },
            doc_comments: DocComments::Visible,
            layout: Layout::OneColumn,
            spacing: Spacing {
                lines_between_scalar_field_comments: 1,
                lines_between_scalar_fields: 0,
                lines_between_tables: 0,
            },
            values_style: ValuesStyle::Field,
            ..Default::default()
        })
    }

    /// Parses and validates a TOML configuration file.
    ///
    /// The TOML document is preserved and returned alongside the config
    /// to enable precise error reporting with source locations.
    pub fn maybe_toml_file(
        source: &str,
        path: &Path,
    ) -> Result<(Config, Document<String>), RenderedDiagnostic> {
        let document = eden_toml::parse_as_document(source, path)?;
        let config: Self = eden_toml::deserialize(&document, path)?;

        config.validate(&ValidationContext {
            source,
            path,
            document: &document,
        })?;

        Ok((config, document))
    }

    /// Standard locations to search for configuration files.
    ///
    /// These paths are platform-specific:
    /// - **Windows**: `%USERPROFILE%/.eden/config.toml`
    /// - **Unix/Linux/macOS**: `/etc/eden/config.toml`
    const ABSOLUTE_CANDIDATES: &[&str] = &[
        #[cfg(windows)]
        "%USERPROFILE%/.eden/config.toml",
        #[cfg(unix)]
        "/etc/eden/config.toml",
    ];

    /// Default configuration file name.
    ///
    /// This file name is used when searching upward through directories
    /// from the current working directory.
    pub const FILE_NAME: &str = "eden.toml";

    /// Searches for the configuration file using a cascading resolution strategy.
    ///
    /// The search follows this priority order:
    ///
    /// 1. **Environment variable**: `EDEN_SETTINGS` (highest priority) or `EDEN_CONFIG_FILE`
    /// 2. **System locations**: Platform-specific standard paths
    ///    - Windows: `%USERPROFILE%/.eden/config.toml`
    ///    - Unix: `/etc/eden/config.toml`
    /// 3. **Current directory walk**: Searches upward from the current directory
    ///    looking for `eden.toml` in each parent directory until it reaches to the root
    #[must_use]
    pub fn find() -> Option<PathBuf> {
        if let Some(path) = var_parsed::<PathBuf>("EDEN_SETTINGS")
            .ok()
            .flatten()
            .or_else(|| var_parsed("EDEN_CONFIG_FILE").ok().flatten())
        {
            return Some(path);
        }

        for candidate in Self::ABSOLUTE_CANDIDATES {
            let path = PathBuf::from(candidate);
            if path.is_file() {
                return Some(path);
            }
        }

        let mut dir = std::env::current_dir().ok();
        while let Some(current) = dir.take() {
            let candidate = current.join(Self::FILE_NAME);
            if candidate.exists() {
                return Some(candidate);
            }
            dir = current.parent().map(Path::to_path_buf);
        }

        None
    }
}
