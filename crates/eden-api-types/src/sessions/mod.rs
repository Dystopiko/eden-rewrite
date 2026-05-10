use eden_minecraft_types::McEdition;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use uuid::fmt::Hyphenated;

use crate::types::EncodedMember;

/// Request body of `POST /sessions`
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RequestSession {
    pub uuid: Hyphenated,
    pub ip: IpAddr,
    pub edition: McEdition,
}

/// Response body of `POST /sessions`
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionGranted {
    pub member: Option<EncodedMember>,
    pub perks: Vec<String>,
}

#[cfg(test)]
mod tests {
    use eden_minecraft_types::McEdition;
    use eden_timestamp::Timestamp;
    use insta::assert_json_snapshot;
    use std::{net::IpAddr, str::FromStr};
    use twilight_model::id::Id;
    use uuid::Uuid;

    use crate::{
        sessions::{RequestSession, SessionGranted},
        types::{EncodedMember, MinimalMemberStatus},
    };

    #[test]
    fn test_serialization() {
        let _guard = crate::testing::setup(&["sessions", "POST"]);

        let body: RequestSession = RequestSession {
            uuid: Uuid::nil().hyphenated(),
            ip: IpAddr::from_str("127.0.0.1").unwrap(),
            edition: McEdition::Java,
        };
        assert_json_snapshot!("request", body);

        let body: SessionGranted = SessionGranted {
            member: Some(EncodedMember {
                id: Id::new(12345),
                name: "steve".to_string(),
                status: Some(MinimalMemberStatus::Okay),
                last_login_at: Some(Timestamp::from_secs(67676767).unwrap()),
                rank: Some("admin".to_string()),
            }),
            perks: vec!["dystopia.instant_restock".into()],
        };
        assert_json_snapshot!("response-okay", body);

        let body: SessionGranted = SessionGranted {
            member: Some(EncodedMember {
                id: Id::new(12345),
                name: "steve".to_string(),
                status: Some(MinimalMemberStatus::Restricted {
                    reason: "griefing".to_string(),
                }),
                last_login_at: Some(Timestamp::from_secs(67676767).unwrap()),
                rank: Some("admin".to_string()),
            }),
            perks: Vec::new(),
        };
        assert_json_snapshot!("response-restricted", body);

        let body: SessionGranted = SessionGranted {
            member: None,
            perks: Vec::new(),
        };
        assert_json_snapshot!("response-guest", body);
    }
}
