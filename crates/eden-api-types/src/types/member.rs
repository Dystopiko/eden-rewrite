use eden_timestamp::Timestamp;
use serde::{Deserialize, Serialize};
use twilight_model::id::{Id, marker::UserMarker};

use crate::types::FullMcAccount;

/// Full metadata of a member that only can be accessed by an administrator.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FullMember {
    pub id: Id<UserMarker>,
    pub name: String,
    pub rank: String,
    pub invited_by: Option<EncodedMember>,
    pub accounts: Vec<FullMcAccount>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EncodedMember {
    pub id: Id<UserMarker>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<MinimalMemberStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_login_at: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MinimalMemberStatus {
    Okay,
    Restricted { reason: String },
}
