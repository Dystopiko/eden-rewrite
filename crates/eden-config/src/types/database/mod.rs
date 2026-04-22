use doku::Document;
use eden_config_derive::Optional;
use eden_file_diagnostics::{RenderedDiagnostic, codespan_reporting::diagnostic::Label};
use eden_sensitive::Sensitive;
use semver::VersionReq;
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};

use crate::{context::SourceContext, validation::Validate};

pub mod common;
pub mod pool;

pub use self::common::Common;
pub use self::pool::DatabasePool;

#[derive(Clone, Debug, Document, Eq, Optional, PartialEq)]
#[optional(attr(derive(Deserialize)))]
#[optional(attr(serde(default)))]
pub struct Database {
    /// Embedded PostgreSQL configuration.
    ///
    /// When configured, Eden will automatically download, install, and manage a PostgreSQL
    /// instance for you. This is useful for development, testing, or simple deployments
    /// where you don't want to manage a separate database server.
    ///
    /// Set to `null` or omit entirely if you're connecting to an external PostgreSQL server.
    #[optional(as = "Option<Embedded>")]
    pub embedded: Option<Embedded>,

    /// Primary database pool handles most reads and writes,
    /// always operating on the latest data revision.
    pub primary: DatabasePool,

    /// Configuration for replica database. This pool should be
    /// optimized for read-heavy workloads.
    #[optional(as = "Option<DatabasePool>")]
    pub replica: Option<DatabasePool>,

    /// Common properties for primary and replica pools.
    #[optional(as = "Common")]
    #[optional(attr(serde(default, flatten)))]
    pub common: Common,
}

impl Default for Database {
    fn default() -> Self {
        let primary = DatabasePool::builder()
            .url(Sensitive::new(
                "host=localhost port=5432 dbname=eden".into(),
            ))
            .readonly(false)
            .build();

        Self {
            embedded: Some(Embedded::default()),
            primary,
            replica: None,
            common: Common::default(),
        }
    }
}

impl<'de> Deserialize<'de> for Database {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let optionals = <OptionalDatabase as Deserialize<'de>>::deserialize(deserializer)?;
        let defaults = Database::default();

        Ok(Self {
            embedded: optionals.embedded,
            primary: optionals.primary.unwrap_or(defaults.primary),
            replica: optionals.replica,
            common: optionals.common,
        })
    }
}

impl Validate for Database {
    fn validate(&self, ctx: &SourceContext<'_>) -> Result<(), RenderedDiagnostic> {
        self.primary.validate(
            &["database", "primary", "min_connections"],
            &["database", "primary", "max_connections"],
            ctx,
        )?;

        let Some((table, replica)) = ctx
            .document
            .get("database")
            .and_then(|v| v.get("replica"))
            .and_then(|v| v.as_table_like())
            .zip(self.replica.as_ref())
        else {
            return Ok(());
        };

        if !replica.readonly {
            let mut builder = ctx.field_diagnostic(
                &["database", "replica", "readonly"],
                "Replica database must not be writable",
            );

            if let Some(span) = table.get("readonly").and_then(|v| v.span()) {
                let label = Label::secondary(0usize, span).with_message("Set readonly to `true`");
                builder = builder.with_label(label);
            }

            builder.emit()?;
        }

        replica.validate(
            &["database", "replica", "min_connections"],
            &["database", "replica", "max_connections"],
            ctx,
        )?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Document, Eq, PartialEq)]
#[serde(default)]
pub struct Embedded {
    /// PostgreSQL version requirement.
    ///
    /// Specify the version requirement of PostgreSQL to use (e.g., "17", "16", "15").
    /// Eden will download and install the appropriate binaries if not already present.
    ///
    /// Default: `"17"`
    #[doku(as = "String", example = "17")]
    pub version: VersionReq,

    /// Directory where PostgreSQL binaries should be installed.
    ///
    /// If not specified, Eden will use a default location based on your operating system.
    /// The directory should be writable and have sufficient space for PostgreSQL binaries
    /// (typically 200-300 MB).
    ///
    /// Default: Platform-specific default location
    pub installation_dir: Option<PathBuf>,

    /// Path to a file containing the database password.
    ///
    /// If specified, Eden will read the password from this file instead of using the
    /// `password` field. This is more secure as it avoids storing passwords directly
    /// in configuration files. The file should contain only the password with no
    /// trailing newline.
    ///
    /// Example: `/run/secrets/eden_db_password`
    pub password_file: Option<PathBuf>,

    /// Directory where PostgreSQL will store its data files.
    ///
    /// This includes all database tables, indexes, and transaction logs. Ensure this
    /// location has sufficient disk space and is backed up regularly. If not specified,
    /// Eden will use a default location.
    ///
    /// Default: Platform-specific default location (e.g., `~/.eden/pgdata`)
    pub data_dir: Option<PathBuf>,

    /// Host address for PostgreSQL to bind to.
    ///
    /// Use "localhost" or "127.0.0.1" for local-only access, or "0.0.0.0" to allow
    /// connections from any network interface. For security, prefer localhost unless
    /// you need external access.
    ///
    /// Default: `"localhost"`
    #[doku(example = "localhost")]
    pub host: String,

    /// Port number for PostgreSQL to listen on.
    ///
    /// The standard PostgreSQL port is 5432. Change this if you have another PostgreSQL
    /// instance running or need to avoid port conflicts.
    ///
    /// Default: `5432`
    #[doku(example = "5432")]
    pub port: u16,

    /// PostgreSQL username for the database.
    ///
    /// This user will be created (or used if it exists) for database connections.
    /// The user will have full privileges on the database.
    ///
    /// Default: `"postgres"`
    #[doku(example = "postgres")]
    pub username: String,

    /// Password for the PostgreSQL user.
    ///
    /// Consider using `password_file` instead for better security, especially in
    /// production environments. This password is used for all database connections.
    ///
    /// Default: `"postgres"` (change this for production!)
    #[doku(as = "String", example = "postgres")]
    pub password: Sensitive<String>,

    /// Whether to drop the database after Eden shuts down.
    ///
    /// When set to `true`, Eden will automatically delete all database data when it
    /// stops. This is useful for testing environments where you want a clean slate
    /// on each run, but should **never** be enabled in production.
    ///
    /// !! Enabling this will result in permanent data loss when Eden stops !!
    ///
    /// Default: `false`
    #[doku(example = "false")]
    pub temporary: bool,

    /// Whether to trust the installation directory.
    ///
    /// If set to `true`, Eden will use PostgreSQL binaries from `installation_dir`
    /// without verification. Only enable this if you're certain the installation
    /// directory contains trusted PostgreSQL binaries.
    ///
    /// Default: `false`
    pub trust_installation_dir: Option<bool>,

    /// Directory for PostgreSQL Unix domain sockets.
    ///
    /// On Unix systems, PostgreSQL can use Unix domain sockets for local connections,
    /// which can be faster and more secure than TCP. If not specified, the system
    /// default socket directory will be used.
    ///
    /// Example: `/var/run/postgresql`
    pub socket_dir: Option<PathBuf>,

    /// Additional PostgreSQL configuration parameters.
    ///
    /// This allows you to set any PostgreSQL configuration parameter (those normally
    /// found in postgresql.conf). Keys should be valid PostgreSQL parameter names.
    ///
    /// ```toml
    /// [database.embedded.configuration]
    /// max_connections = "100"
    /// shared_buffers = "256MB"
    /// work_mem = "4MB"
    /// maintenance_work_mem = "64MB"
    /// ```
    pub configuration: HashMap<String, String>,
}

impl Default for Embedded {
    fn default() -> Self {
        Self {
            version: VersionReq::parse("17").expect("should be a valid semver version requirement"),
            installation_dir: None,
            password_file: None,
            data_dir: None,
            host: String::from("localhost"),
            port: 5432,
            username: "postgres".into(),
            password: Sensitive::new("postgres".into()),
            temporary: false,
            trust_installation_dir: None,
            socket_dir: None,
            configuration: HashMap::new(),
        }
    }
}
