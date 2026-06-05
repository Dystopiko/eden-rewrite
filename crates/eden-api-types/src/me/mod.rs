use eden_timestamp::Timestamp;
use serde::{Deserialize, Serialize};
use twilight_model::id::{Id, marker::UserMarker};

/// Response body of `GET /me`
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CurrentUser {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id<UserMarker>>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<String>,
    pub last_used_at: Option<Timestamp>,
}

#[cfg(test)]
mod tests {
    use eden_timestamp::Timestamp;
    use insta::assert_json_snapshot;
    use twilight_model::id::Id;

    use crate::me::CurrentUser;

    #[test]
    fn test_serialization_for_members() {
        let _guard = crate::testing::setup(&["me", "GET", "member"]);
        let body: CurrentUser = CurrentUser {
            id: Some(Id::new(123456)),
            name: "steve".to_string(),
            rank: Some("member".to_string()),
            last_used_at: Timestamp::from_secs(1238942).ok(),
        };
        assert_json_snapshot!("response", body);
    }

    #[test]
    fn test_serialization_for_mc_servers() {
        let _guard = crate::testing::setup(&["me", "GET", "mc_server"]);
        let body: CurrentUser = CurrentUser {
            id: None,
            name: "steve".to_string(),
            rank: None,
            last_used_at: Timestamp::from_secs(1238942).ok(),
        };
        assert_json_snapshot!("response", body);
    }
}
