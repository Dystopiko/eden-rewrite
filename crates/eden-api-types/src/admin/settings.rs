use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PatchSettings {
    pub allow_guests: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::PatchSettings;
    use insta::assert_json_snapshot;

    #[test]
    fn test_serialization_of_patch_settings() {
        let _guard = crate::testing::setup(&["admin", "settings", "PATCH"]);
        let patch = PatchSettings {
            allow_guests: Some(true),
        };
        assert_json_snapshot!("request", patch);
    }
}
