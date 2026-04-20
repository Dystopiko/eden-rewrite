use constant_time_eq::constant_time_eq;
use doku::Document;
use eden_config_derive::Optional;
use eden_sensitive::Sensitive;
use serde::Deserialize;
use std::{borrow::Cow, fmt, num::NonZeroU32};

/// Configuration for a single SQLite connection pool.
///
/// Controls the connection URL, pool sizing, and whether the pool
/// should enforce read-only access at the connection level.
#[derive(Clone, Debug, Deserialize, Document, Optional, PartialEq, Eq)]
#[optional(vis = pub(super))]
#[optional(attr(derive(Deserialize)))]
pub struct DatabasePool {
    /// SQLite connection URL for the database pool.
    #[doku(as = "String", example = ":memory:")]
    pub url: SqliteUrl,

    /// Minimum number of connections to keep open.
    pub min_connections: u32,

    /// Maximum number of connections allowed.
    #[doku(as = "u32", example = "1")]
    pub max_connections: NonZeroU32,

    /// Set to true to make this pool read-only. Not recommended
    /// for the primary pool.
    pub readonly: bool,
}

/// An SQLite connection URL, either file-backed or in-memory.
#[derive(Clone)]
pub struct SqliteUrl {
    inner: Sensitive<Cow<'static, str>>,
}

impl SqliteUrl {
    pub const MEMORY: Self = Self {
        inner: Sensitive::new(Cow::Borrowed(":memory:")),
    };

    /// Leaks the redacted object and it returns a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.inner
    }
}

impl Default for SqliteUrl {
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
