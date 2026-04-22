use bon::Builder;
use doku::Document;
use eden_config_derive::Optional;
use eden_file_diagnostics::{RenderedDiagnostic, codespan_reporting::diagnostic::Label};
use eden_sensitive::Sensitive;
use serde::Deserialize;
use std::num::NonZeroU32;

use crate::context::SourceContext;

/// Database connection pool configuration.
///
/// Configures how Eden connects to and maintains connections with your PostgreSQL database.
/// Connection pooling improves performance by reusing database connections instead of
/// creating new ones for each operation.
#[derive(Builder, Clone, Debug, Deserialize, Document, Eq, Optional, PartialEq)]
pub struct DatabasePool {
    /// PostgreSQL connection URL for the database pool.
    #[doku(as = "String", example = "postgresql://user@secret:localhost/eden")]
    pub url: Sensitive<String>,

    /// Minimum number of connections to keep open.
    ///
    /// Eden will maintain at least this many connections to the database at all times,
    /// even during periods of low activity. This ensures quick response times by
    /// avoiding connection setup overhead.
    ///
    /// Default: `0`
    #[builder(default = 0)]
    pub min_connections: u32,

    /// Maximum number of connections allowed.
    ///
    /// This is the upper limit of concurrent database connections Eden can create.
    /// When all connections are in use, new requests will wait for a connection to
    /// become available.
    ///
    /// **Important:** Ensure your PostgreSQL server's `max_connections` setting is
    /// higher than the sum of all your application pools' `max_connections` values.
    ///
    /// Default: `3`
    #[builder(default = NonZeroU32::new(3).expect("three is less than zero"))]
    #[doku(as = "u32", example = "1")]
    pub max_connections: NonZeroU32,

    /// Set to true to make this pool read-only. Not recommended
    /// for the primary pool.
    ///
    /// When enabled, this pool will reject any write operations (INSERT, UPDATE, DELETE).
    /// This is a safety feature to prevent accidental writes to replica databases.
    ///
    /// **Required:** Must be `true` for replica pools (validated by Eden)
    /// **Primary pool:** Should be `false` (default)
    ///
    /// Default: `false`
    #[builder(default = false)]
    pub readonly: bool,
}

impl<S: database_pool_builder::State> DatabasePoolBuilder<S> {
    pub fn empty_url(self) -> DatabasePoolBuilder<database_pool_builder::SetUrl<S>>
    where
        S::Url: database_pool_builder::IsUnset,
    {
        self.url("".to_string().into())
    }
}

impl DatabasePool {
    pub(super) fn validate(
        &self,
        min_path: &[&str],
        max_path: &[&str],
        ctx: &SourceContext,
    ) -> Result<(), RenderedDiagnostic> {
        if self.min_connections > self.max_connections.get() {
            let max_span = max_path
                .iter()
                .try_fold(ctx.document.as_item(), |item, key| item.get(key))
                .and_then(|v| v.span());

            let mut builder = ctx.field_diagnostic(
                min_path,
                "`min_connections` must not be greater than `max_connections`!",
            );

            if let Some(span) = max_span {
                let label = Label::primary(0usize, span);
                builder = builder.with_label(label);
            }

            builder.emit()?;
        }

        Ok(())
    }
}
