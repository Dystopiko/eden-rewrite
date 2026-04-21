use bon::Builder;
use constant_time_eq::constant_time_eq;
use doku::Document;
use eden_config_derive::Optional;
use eden_file_diagnostics::{RenderedDiagnostic, codespan_reporting::diagnostic::Label};
use eden_sensitive::Sensitive;
use serde::Deserialize;
use std::{borrow::Cow, fmt, num::NonZeroU32};

use crate::context::SourceContext;

/// Configuration for a single SQLite connection pool.
///
/// Controls the connection URL, pool sizing, and whether the pool
/// should enforce read-only access at the connection level.
///
/// You may use the [`builder`] function to configure a customized
/// database pool configuration for your needs.
#[derive(Builder, Clone, Debug, Deserialize, Document, Optional, PartialEq, Eq)]
pub struct DatabasePool {
    /// SQLite connection URL for the database pool.
    #[builder(default = SqliteUrl::MEMORY)]
    #[doku(as = "String", example = ":memory:")]
    pub url: SqliteUrl,

    /// Minimum number of connections to keep open.
    #[builder(default = 0)]
    pub min_connections: u32,

    /// Maximum number of connections allowed.
    #[builder(default = NonZeroU32::new(3).expect("three is less than zero"))]
    #[doku(as = "u32", example = "1")]
    pub max_connections: NonZeroU32,

    /// Set to true to make this pool read-only. Not recommended
    /// for the primary pool.
    #[builder(default = false)]
    pub readonly: bool,
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

/// An SQLite connection URL, either file-backed or in-memory.
#[derive(Clone)]
pub struct SqliteUrl {
    inner: Sensitive<Cow<'static, str>>,
}

impl SqliteUrl {
    /// A constant SQLite URL for in-memory databases.
    ///
    /// This is a special SQLite URL that creates a temporary database
    /// in memory rather than on disk. The database exists only for the
    /// duration of the connection and is automatically destroyed when
    /// the connection is closed.
    pub const MEMORY: Self = Self::from_static(":memory:");

    /// Creates a new [`SqliteUrl`] from an owned [`String`].
    #[must_use]
    pub fn from_owned(url: String) -> Self {
        let inner = Sensitive::new(Cow::Owned(url));
        Self { inner }
    }

    /// Creates a new [`SqliteUrl`] from a static string reference.
    ///
    /// This is a const function that allows creating [`SqliteUrl`] instances
    /// at compile time. The URL is stored as a borrowed reference without
    /// allocation.
    #[must_use]
    pub const fn from_static(url: &'static str) -> Self {
        let inner = Sensitive::new(Cow::Borrowed(url));
        Self { inner }
    }

    /// Returns the underlying URL as a string slice.
    ///
    /// Note: This exposes the sensitive connection URL. Use with caution
    /// and avoid logging or displaying the returned value directly.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.inner
    }
}

impl Default for SqliteUrl {
    /// It provides the value of [`SqliteUrl::MEMORY`]
    fn default() -> Self {
        Self::MEMORY
    }
}

impl PartialEq for SqliteUrl {
    fn eq(&self, other: &Self) -> bool {
        constant_time_eq(self.as_str().as_bytes(), other.as_str().as_bytes())
    }
}

impl Eq for SqliteUrl {}

impl fmt::Debug for SqliteUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SqliteUrl(..)")
    }
}

impl fmt::Display for SqliteUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

struct SqliteUrlVisitor;

impl<'de> serde::de::Visitor<'de> for SqliteUrlVisitor {
    type Value = SqliteUrl;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an SQLite connection URL or \":memory:\"")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if v.is_empty() {
            return Err(serde::de::Error::custom("SQLite URL should not be empty"));
        }

        Ok(SqliteUrl {
            inner: Sensitive::new(Cow::Owned(v.to_string())),
        })
    }
}

impl<'de> Deserialize<'de> for SqliteUrl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(SqliteUrlVisitor)
    }
}
