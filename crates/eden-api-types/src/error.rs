use serde::{Deserialize, Serialize};

/// A serializable error response returned to API clients.
///
/// It contains an optional [`Uuid`] that correlates the response with server-side logs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Error {
    pub code: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use crate::error::Error;
    use insta::assert_json_snapshot;

    #[test]
    fn test_serialization() {
        let _guard = crate::testing::setup(&["error"]);
        let error = Error {
            code: "READONLY_MODE".to_string(),
            message: "Readonly mode".to_string(),
        };
        assert_json_snapshot!(error);
    }
}
