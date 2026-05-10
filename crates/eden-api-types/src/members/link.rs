use eden_minecraft_types::McEdition;
use eden_timestamp::Timestamp;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use uuid::fmt::Hyphenated;

/// Request body of `POST /members/link`
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LinkMcAccount {
    pub uuid: Hyphenated,
    pub username: String,
    pub ip: IpAddr,
    pub edition: McEdition,
}

/// Response body of `POST /members/link`
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LinkMcAccountChallenge {
    pub code: String,
    pub expires_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use eden_minecraft_types::McEdition;
    use eden_timestamp::Timestamp;
    use insta::assert_json_snapshot;
    use std::{net::IpAddr, str::FromStr};
    use uuid::Uuid;

    use crate::members::link::{LinkMcAccount, LinkMcAccountChallenge};

    #[test]
    fn test_serialization() {
        let _guard = crate::testing::setup(&["members", "link", "POST"]);

        let body = LinkMcAccount {
            uuid: Uuid::nil().hyphenated(),
            username: "steve".to_string(),
            ip: IpAddr::from_str("127.0.0.1").unwrap(),
            edition: McEdition::Java,
        };
        assert_json_snapshot!("request", body);

        let body = LinkMcAccountChallenge {
            code: "alice-bob-charlie".to_string(),
            expires_at: Timestamp::from_str("2026-12-18T00:00:00Z").unwrap(),
        };
        assert_json_snapshot!("response", body);
    }
}
