use crate::{context::SourceContext, validation::Validate};
use doku::Document;
use eden_file_diagnostics::{RenderedDiagnostic, codespan_reporting::diagnostic::Label};
use serde::Deserialize;

pub mod pool;
pub use self::pool::{DatabasePool, SqliteUrl};

#[derive(Clone, Debug, Document, PartialEq, Eq)]
// #[serde(default)]
pub struct Database {
    /// Primary database pool handles most reads and writes,
    /// always operating on the latest data revision.
    // #[serde()]
    pub primary: DatabasePool,

    /// Configuration for replica database. This pool should be
    /// optimized for read-heavy workloads.
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

impl Validate for Database {
    fn validate(&self, ctx: &SourceContext<'_>) -> Result<(), RenderedDiagnostic> {
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

        Ok(())
    }
}

mod default_primary_pool {
    use serde::Deserializer;

    use crate::types::database::{DatabasePool, pool::OptionalDatabasePool};

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<DatabasePool, D::Error>
    where
        D: Deserializer<'de>,
    {
        todo!()
    }
}
