use crate::{context::SourceContext, validation::Validate};

use doku::Document;
use eden_config_derive::Optional;
use eden_file_diagnostics::{RenderedDiagnostic, codespan_reporting::diagnostic::Label};
use serde::Deserialize;
use std::num::NonZeroU32;

pub mod pool;
pub use self::pool::{DatabasePool, SqliteUrl};

#[derive(Clone, Debug, Document, Optional, PartialEq, Eq)]
#[optional(attr(derive(Deserialize)))]
#[optional(attr(serde(default)))]
pub struct Database {
    /// Primary database pool handles most reads and writes,
    /// always operating on the latest data revision.
    pub primary: DatabasePool,

    /// Configuration for replica database. This pool should be
    /// optimized for read-heavy workloads.
    #[optional(as = "Option<DatabasePool>")]
    pub replica: Option<DatabasePool>,

    // Background jobs also make heavy use of transactions, which hold
    // connections for longer and amplify contention.
    //
    // In practice, connection acquisition averaged 4 ms and some
    // Gateway API routes reached 20–200 ms under shared load.
    //
    /// Configuration for dedicated background jobs database.
    ///
    /// Keeping background jobs on a separate pool prevents them from
    /// competing with the primary and replica pools for connections.
    pub background_jobs: DatabasePool,
}

impl<'de> Deserialize<'de> for Database {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let optionals = <OptionalDatabase as Deserialize<'de>>::deserialize(deserializer)?;
        let defaults = Database::default();
        debug_assert_eq!(defaults.replica, None);

        Ok(Self {
            primary: optionals.primary.unwrap_or(defaults.primary),
            replica: optionals.replica,
            background_jobs: optionals
                .background_jobs
                .unwrap_or(defaults.background_jobs),
        })
    }
}

impl Default for Database {
    fn default() -> Self {
        Self {
            primary: DatabasePool {
                url: SqliteUrl::MEMORY,
                min_connections: 0,
                max_connections: NonZeroU32::new(3).expect("three is greater than zero"),
                readonly: false,
            },
            replica: None,
            background_jobs: DatabasePool {
                url: SqliteUrl::from_static("sqlite://./background_jobs.db"),
                min_connections: 0,
                max_connections: NonZeroU32::new(3).expect("three is greater than zero"),
                readonly: false,
            },
        }
    }
}

impl Validate for Database {
    fn validate(&self, ctx: &SourceContext<'_>) -> Result<(), RenderedDiagnostic> {
        self.primary.validate(
            &["database", "primary", "min_connections"],
            &["database", "primary", "max_connections"],
            ctx,
        )?;

        self.background_jobs.validate(
            &["database", "background_jobs", "min_connections"],
            &["database", "background_jobs", "max_connections"],
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
