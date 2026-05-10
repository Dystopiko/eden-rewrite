use serde::{Deserialize, Serialize};

/// A serializable error response returned to API clients.
///
/// It contains a machine-readable [`ErrorCode`], a human-readable message, and an optional
/// [`Uuid`] that correlates the response with server-side logs.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
}

/// A machine-readable classification of an API error.
///
/// Serialized as `SCREAMING_SNAKE_CASE` in JSON responses (e.g. `"NOT_FOUND"`),
/// and mapped to an appropriate HTTP status code via [`From<ErrorCode> for StatusCode`].
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// Maps to `500 Internal Server Error`.
    Internal,
    /// Maps to `503 Service Unavailable`.
    ReadonlyMode,
    /// Maps to `404 Not Found`.
    NotFound,
    /// Maps to `400 Bad Request`.
    InvalidRequest,
    /// Maps to `503 Service Unavailable`.
    ServiceUnavailable,
    /// Maps to `429 Too Many Requests`
    RateLimited,
    /// Maps to `401 Unauthorized`.
    Unauthorized,
    /// Maps to `403 Forbidden`
    Forbidden,
}

#[cfg(test)]
mod tests {
    use crate::error::{Error, ErrorCode};
    use insta::assert_json_snapshot;

    #[test]
    fn test_serialization() {
        let _guard = crate::testing::setup(&["error"]);
        let error = Error {
            code: ErrorCode::ReadonlyMode,
            message: "Readonly mode".to_string(),
        };
        assert_json_snapshot!(error);
    }
}
