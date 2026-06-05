use eden_timestamp::Timestamp;
use serde::{Deserialize, Serialize};
use twilight_model::id::{Id, marker::UserMarker};

/// Request body of `POST /auth/login`
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Login {
    pub member: Id<UserMarker>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub totp: Option<String>,
}

/// Successful response body of `POST /auth/login`
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionGranted {
    pub token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Timestamp>,
}

#[cfg(test)]
mod tests {
    use insta::assert_json_snapshot;
    use twilight_model::id::Id;

    use crate::auth::login::Login;

    #[test]
    fn test_serialization() {
        let _guard = crate::testing::setup(&["auth", "login", "POST"]);
        let body = Login {
            member: Id::new(123456),
            totp: None,
        };
        assert_json_snapshot!("request", body);
    }
}
