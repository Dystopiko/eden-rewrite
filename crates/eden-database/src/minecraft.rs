use serde::{Deserialize, Serialize};

/// Differentiates between Minecraft editions.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "mc_account_type", rename_all = "lowercase")]
pub enum McAccountType {
    Java,
    Bedrock,
}

impl McAccountType {
    /// Returns `true` if this account type is a Java edition account.
    #[must_use]
    pub const fn is_java(&self) -> bool {
        matches!(self, McAccountType::Java)
    }

    /// Returns `true` if this account type is a Bedrock edition account.
    #[must_use]
    pub const fn is_bedrock(&self) -> bool {
        matches!(self, McAccountType::Bedrock)
    }
}

impl std::fmt::Display for McAccountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Java => f.write_str("java"),
            Self::Bedrock => f.write_str("bedrock"),
        }
    }
}
