use serde::{Deserialize, Serialize};
use std::{borrow::Cow, fmt, str::FromStr};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ResourceKey {
    pub(super) registry: Cow<'static, str>,
    pub(super) identifier: Cow<'static, str>,
}

impl ResourceKey {
    #[must_use]
    pub fn custom(registry: &str, identifier: &str) -> Option<Self> {
        if !registry.chars().all(is_valid_resource_part_char) {
            return None;
        }

        if !identifier.chars().all(is_valid_resource_part_char) {
            return None;
        }

        Some(Self {
            registry: Cow::Owned(registry.to_owned()),
            identifier: Cow::Owned(identifier.to_owned()),
        })
    }

    #[must_use]
    pub fn registry(&self) -> &str {
        &self.registry
    }

    #[must_use]
    pub fn identifier(&self) -> &str {
        &self.identifier
    }
}

impl fmt::Display for ResourceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.registry, self.identifier)
    }
}

impl<'de> Deserialize<'de> for ResourceKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(ResourceKeyVisitor)
    }
}

impl Serialize for ResourceKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ResourceKeyParseError {
    MissingColon,
    MissingSpecifier,
    MissingScope,
    FoundIllegalCharacter,
}

impl fmt::Display for ResourceKeyParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FoundIllegalCharacter => f.write_str("found illegal character"),
            Self::MissingColon => f.write_str("missing ':'"),
            Self::MissingSpecifier => f.write_str("missing dimension specifier"),
            Self::MissingScope => f.write_str("missing dimension scope"),
        }
    }
}

impl std::error::Error for ResourceKeyParseError {}

impl FromStr for ResourceKey {
    type Err = ResourceKeyParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (registry, rest) = match s.find(':') {
            None => return Err(ResourceKeyParseError::MissingColon),
            Some(i) => s.split_at(i),
        };

        if registry.is_empty() {
            return Err(ResourceKeyParseError::MissingScope);
        }

        let identifier = &rest[1..];
        if identifier.is_empty() {
            return Err(ResourceKeyParseError::MissingSpecifier);
        }

        if !registry.chars().all(is_valid_resource_part_char)
            || !identifier.chars().all(is_valid_resource_part_char)
        {
            return Err(ResourceKeyParseError::FoundIllegalCharacter);
        }

        Ok(Self {
            registry: Cow::Owned(registry.to_string()),
            identifier: Cow::Owned(identifier.to_string()),
        })
    }
}

struct ResourceKeyVisitor;

impl<'de> serde::de::Visitor<'de> for ResourceKeyVisitor {
    type Value = ResourceKey;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Minecraft resource key")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        ResourceKey::from_str(v).map_err(serde::de::Error::custom)
    }
}

// Allow lowercase ASCII letters, digits, and underscores.
//
// This composes of full set of characters valid in a
// Minecraft resource key path segment.
const fn is_valid_resource_part_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Dimension;

    #[test]
    fn test_mc_resource_key_serialization() {
        let possible_values = &[
            &*Dimension::OVERWORLD,
            &*Dimension::THE_END,
            &*Dimension::THE_NETHER,
            &ResourceKey::custom("hello", "world").unwrap(),
        ];
        insta::assert_json_snapshot!(possible_values);
    }

    #[test]
    fn test_display_fmt() {
        assert_eq!(Dimension::OVERWORLD.to_string(), "minecraft:overworld");
        assert_eq!(Dimension::THE_NETHER.to_string(), "minecraft:the_nether");
        assert_eq!(Dimension::THE_END.to_string(), "minecraft:the_end");
        assert_eq!(
            ResourceKey::custom("hello", "world").unwrap().to_string(),
            "hello:world"
        );
    }

    #[test]
    fn should_parse_from_str() {
        let dim: ResourceKey = "mymod:custom_dim".parse().unwrap();
        let expected = ResourceKey::custom("mymod", "custom_dim").unwrap();
        assert_eq!(dim, expected);
    }
}
