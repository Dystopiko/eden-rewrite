use doku::Document;
use eden_config_derive::Validate;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str::FromStr};
use thiserror::Error;
use twilight_model::id::{Id, marker::GuildMarker};
use uuid::{Uuid, fmt::Hyphenated};

#[derive(Clone, Debug, Default, Deserialize, Document, Eq, PartialEq, Validate)]
#[serde(default)]
pub struct Minecraft {
    /// Minecraft permission management.
    ///
    /// Define LuckPerms permission nodes to be automatically granted to players
    /// based on their role. Supports named player group and player-specific targeting.
    ///
    /// ```toml
    /// # Named groups
    /// contributors = ["veinminer"]
    /// member       = ["dystopia.instantrestock"]
    /// staff        = []
    /// admins       = []
    ///
    /// # Player-specific overrides:
    /// "745809834183753828" = ["deadchest"]                    # Discord snowflake
    /// "a1705729-8729-3a49-befe-0ee68d88a374" = ["veinminer"]  # Minecraft UUID
    /// ```
    #[doku(as = "HashMap<String, Vec<String>>", example = "")]
    #[validate(skip)]
    // IndexMap preserves insertion order, which keeps insta snapshots
    // deterministic across all test scenarios. Its performance
    // characteristics are comparable to HashMap.
    pub perks: IndexMap<PerkId, Vec<String>>,
}

/// Identifies the target of a perk assignment in the [`Minecraft::perks`] map.
///
/// A `PerkId` is parsed from a TOML string key and can represent either a
/// named player group or a specific player identified by a Discord snowflake
/// or a Minecraft UUID.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum PerkId {
    /// Targets a specific player identified by their Discord user snowflake ID.
    Discord(Id<GuildMarker>),

    /// Targets a specific player identified by their Minecraft account UUID.
    Uuid(Uuid),

    /// Targets all players with the `"member"` role.
    Members,

    /// Targets all players with the `"contributor"` role.
    Contributors,

    /// Targets all players with the `"staff"` role.
    Staff,

    /// Targets all players with the `"admin"` role.
    Admins,
}

impl<'de> Deserialize<'de> for PerkId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(PerkIdVisitor)
    }
}

struct PerkIdVisitor;

impl<'de> serde::de::Visitor<'de> for PerkIdVisitor {
    type Value = PerkId;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "perk identifier: a named group (\"admins\", \"contributors\", \
            \"staff\", \"members\"), a Discord snowflake, or a hyphenated \
            Minecraft UUID",
        )
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        v.parse().map_err(E::custom)
    }
}

/// Error returned when a string cannot be parsed into a [`PerkId`].
#[derive(Debug, Error)]
pub enum PerkIdParseError {
    /// The string contained only digits but is not a valid Discord snowflake.
    #[error("invalid Discord snowflake: {0}")]
    InvalidSnowflake(String),

    /// The string looked like a UUID (hex digits and hyphens) but failed to parse.
    #[error("invalid Minecraft UUID: {0}")]
    InvalidUuid(String),

    /// The string did not match any named group or known identifier format.
    #[error(
        "unknown perk identifier `{0}`; expected one of \"admins\", \
        \"contributors\", \"staff\", \"members\", a Discord snowflake, \
        or a hyphenated Minecraft UUID"
    )]
    Unknown(String),
}

impl FromStr for PerkId {
    type Err = PerkIdParseError;

    fn from_str(v: &str) -> Result<Self, Self::Err> {
        // Discord snowflake: all ASCII digits.
        if v.chars().all(|c| c.is_ascii_digit()) {
            return v
                .parse()
                .map(PerkId::Discord)
                .map_err(|_| PerkIdParseError::InvalidSnowflake(v.to_owned()));
        }

        // Minecraft UUID: hex digits and hyphens.
        if v.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
            return Hyphenated::from_str(v)
                .map(|h| PerkId::Uuid(h.into_uuid()))
                .map_err(|_| PerkIdParseError::InvalidUuid(v.to_owned()));
        }

        match v {
            "admins" => Ok(PerkId::Admins),
            "contributors" => Ok(PerkId::Contributors),
            "staff" => Ok(PerkId::Staff),
            "members" => Ok(PerkId::Members),
            _ => Err(PerkIdParseError::Unknown(v.to_owned())),
        }
    }
}

impl Serialize for PerkId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            PerkId::Discord(id) => id.serialize(serializer),
            PerkId::Uuid(uuid) => Hyphenated::from_uuid(*uuid).serialize(serializer),
            PerkId::Members => serializer.serialize_str("members"),
            PerkId::Contributors => serializer.serialize_str("contributors"),
            PerkId::Staff => serializer.serialize_str("staff"),
            PerkId::Admins => serializer.serialize_str("admins"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::types::organization::minecraft::PerkId;
    use serde_json::from_str;
    use serde_test::{Token, assert_tokens};
    use std::str::FromStr;
    use twilight_model::id::Id;
    use uuid::Uuid;

    #[test]
    fn test_serde() {
        assert_tokens(&PerkId::Admins, &[Token::Str("admins")]);
        assert_tokens(&PerkId::Contributors, &[Token::Str("contributors")]);
        assert_tokens(&PerkId::Members, &[Token::Str("members")]);
        assert_tokens(&PerkId::Staff, &[Token::Str("staff")]);

        let id = from_str::<PerkId>(r#""12345""#).unwrap();
        assert_eq!(id, PerkId::Discord(Id::new(12345)));

        let uuid = Uuid::from_str("066d6b95-43fc-4566-9eb1-54967c8ed5b3").unwrap();
        let id = from_str::<PerkId>(r#""066d6b95-43fc-4566-9eb1-54967c8ed5b3""#).unwrap();
        assert_eq!(id, PerkId::Uuid(uuid));
    }
}
