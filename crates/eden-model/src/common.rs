use serde::{Deserialize, Serialize};
use sqlx::Type;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "approval_status", rename_all = "lowercase")]
pub enum ApprovalStatus {
    #[default]
    Pending,
    Approved,
    Revoked,
}
