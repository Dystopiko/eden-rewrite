use eden_timestamp::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PatchSettings {
    pub allow_guests: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EncodedSettings {
    pub allow_guests: bool,
    pub updated_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::{EncodedSettings, PatchSettings};
    use eden_timestamp::Timestamp;
    use insta::assert_json_snapshot;

    #[test]
    fn test_serialization_of_encoded_settings() {
        let _guard = crate::testing::setup(&["admin", "settings", "GET"]);
        let patch = EncodedSettings {
            allow_guests: true,
            updated_at: Timestamp::from_secs(123).unwrap(),
        };
        assert_json_snapshot!("response", patch);
    }

    #[test]
    fn test_serialization_of_patch_settings() {
        let _guard = crate::testing::setup(&["admin", "settings", "PATCH"]);
        let patch = PatchSettings {
            allow_guests: Some(true),
        };
        assert_json_snapshot!("request", patch);
    }
}
