use eden_minecraft_types::McEdition;
use serde::{Deserialize, Serialize};
use twilight_model::id::{Id, marker::UserMarker};
use uuid::fmt::Hyphenated;

/// Request body of `POST /admin/members/{id}/link`
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LinkMcAccount {
    pub edition: McEdition,
    pub uuid: Hyphenated,
    pub username: String,
}

/// Request body of `PATCH /admin/members/{id}`
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PatchMember {
    pub name: Option<String>,
    pub invited_by: Option<Id<UserMarker>>,
}

#[cfg(test)]
mod tests {
    use eden_minecraft_types::McEdition;
    use insta::assert_json_snapshot;
    use uuid::Uuid;

    use crate::admin::members::{LinkMcAccount, PatchMember};

    #[test]
    fn test_serialization_of_link_mc_account() {
        let _guard = crate::testing::setup(&["admin", "members", "id", "link", "POST"]);

        let body: LinkMcAccount = LinkMcAccount {
            edition: McEdition::Java,
            uuid: Uuid::nil().hyphenated(),
            username: "steve".to_string(),
        };
        assert_json_snapshot!("request", body);
    }

    #[test]
    fn test_serialization_of_patch_member() {
        let _guard = crate::testing::setup(&["admin", "members", "id", "PATCH"]);

        let body: PatchMember = PatchMember {
            name: Some("alex".to_string()),
            invited_by: None,
        };
        assert_json_snapshot!("request", body);
    }
}
