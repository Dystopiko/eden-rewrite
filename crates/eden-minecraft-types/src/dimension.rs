use serde::{Deserialize, Serialize};
use std::{borrow::Cow, fmt, ops::Deref, str::FromStr};

use crate::{ResourceKey, resource_key::ResourceKeyParseError};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Dimension(ResourceKey);

impl Dimension {
    // https://minecraft.wiki/w/Dimension
    pub const OVERWORLD: Self = Self(ResourceKey {
        registry: Cow::Borrowed("minecraft"),
        identifier: Cow::Borrowed("overworld"),
    });

    pub const THE_NETHER: Self = Self(ResourceKey {
        registry: Cow::Borrowed("minecraft"),
        identifier: Cow::Borrowed("the_nether"),
    });

    pub const THE_END: Self = Self(ResourceKey {
        registry: Cow::Borrowed("minecraft"),
        identifier: Cow::Borrowed("the_end"),
    });

    #[must_use]
    pub const fn resource_key(&self) -> &ResourceKey {
        &self.0
    }
}

impl From<ResourceKey> for Dimension {
    fn from(value: ResourceKey) -> Self {
        Self(value)
    }
}

impl From<Dimension> for ResourceKey {
    fn from(value: Dimension) -> Self {
        value.0
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DimensionParseError {
    UnknownMcDimension,
    ResourceKey(ResourceKeyParseError),
}

impl From<ResourceKeyParseError> for DimensionParseError {
    fn from(value: ResourceKeyParseError) -> Self {
        Self::ResourceKey(value)
    }
}

impl fmt::Display for DimensionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownMcDimension => f.write_str("unknown Minecraft dimension"),
            Self::ResourceKey(inner) => fmt::Display::fmt(&inner, f),
        }
    }
}

impl std::error::Error for DimensionParseError {}

impl<'de> Deserialize<'de> for Dimension {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(DimensionVisitor)
    }
}

impl Serialize for Dimension {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&**self)
    }
}

impl Deref for Dimension {
    type Target = ResourceKey;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for Dimension {
    type Err = DimensionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let key = ResourceKey::from_str(s)?;
        if key.registry() == "minecraft" {
            return Ok(match key.identifier() {
                "overworld" => Dimension::OVERWORLD,
                "the_nether" => Dimension::THE_NETHER,
                "the_end" => Dimension::THE_END,
                _ => return Err(DimensionParseError::UnknownMcDimension),
            });
        }
        Ok(Self(key))
    }
}

struct DimensionVisitor;

impl<'de> serde::de::Visitor<'de> for DimensionVisitor {
    type Value = Dimension;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Minecraft dimension resource key")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Dimension::from_str(v).map_err(serde::de::Error::custom)
    }
}
