use constant_time_eq::constant_time_eq;
use serde::Deserialize;

/// A secure wrapper for a Discord bot authorization token allocated in the heap
/// that redacts the entire string if used with [`Debug`] and [`Display`].
///
/// It also provides constant-time equality comparison to prevent timing attacks
///
/// The user is responsible for handling the token and avoiding the
/// token from being exposed in the stack memory.
///
/// [`Display`]: std::fmt::Display
#[derive(Clone, Default)]
pub struct Token {
    inner: Box<str>,
}

impl Token {
    /// Creates a new [`Token`] wrapping `value`.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        let inner = value.into().into_boxed_str();
        Self { inner }
    }

    /// Returns the raw token value as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.inner
    }
}

impl PartialEq for Token {
    /// Compares tokens in constant time to prevent timing side-channels.
    fn eq(&self, other: &Self) -> bool {
        constant_time_eq(self.inner.as_bytes(), other.inner.as_bytes())
    }
}

impl Eq for Token {}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Token").finish_non_exhaustive()
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

impl<'de> Deserialize<'de> for Token {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Token;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("Discord bot token string")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Token::new(v))
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}
