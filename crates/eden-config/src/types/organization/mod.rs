use doku::Document;
use eden_config_derive::{Optional, Validate};
use heck::ToSnakeCase;
use serde::Deserialize;

mod validators;

pub mod discord;
pub mod minecraft;

pub use self::discord::Discord;
pub use self::minecraft::Minecraft;

#[derive(Clone, Debug, Document, Eq, Optional, PartialEq, Validate)]
#[optional(attr(derive(Deserialize)))]
#[optional(attr(serde(default)))]
pub struct Organization {
    /// The name of the organization.
    ///
    /// This field is required as Eden will customize all of its messages
    /// catered to your organization.
    ///
    /// If not specified, the default value is `Dystopia`.
    #[validate(skip)]
    pub name: String,

    /// The identifier of the organization.
    ///
    /// This field is optional as Eden will automatically convert the
    /// organization name to lowercased.
    #[validate(with = "self::validators::validate_org_identifier")]
    pub identifier: String,

    /// Discord configuration related to the organization's Discord guild (server).
    ///
    /// If this table is missing, Discord bot service will be disabled
    /// automatically and any incoming messages will not be processed
    /// internally.
    pub discord: Option<Discord>,

    /// Minecraft server management configuration.
    #[optional(as = "Minecraft")]
    pub minecraft: Minecraft,
}

impl Default for Organization {
    fn default() -> Self {
        Self {
            identifier: "dystopia".to_string(),
            name: "Dystopia".to_string(),
            discord: None,
            minecraft: Minecraft::default(),
        }
    }
}

impl<'de> Deserialize<'de> for Organization {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut optional = OptionalOrganization::deserialize(deserializer)?;
        let defaults = Organization::default();

        let is_name_blank = optional
            .name
            .as_ref()
            .map(|v| v.is_empty() || v.chars().all(|v| v.is_whitespace()))
            .unwrap_or(true);

        if is_name_blank {
            optional.name = Some(defaults.name.to_string());
            optional.identifier = Some(defaults.identifier.to_string());
        } else if optional.identifier.is_none() {
            let name = optional
                .name
                .as_ref()
                .expect("organization.name should exists");

            optional.identifier = Some(name.to_snake_case());
        }

        Ok(Organization {
            name: optional
                .name
                .expect("name should be overriden with default one"),
            identifier: optional
                .identifier
                .expect("identifier should be overriden with default one"),
            discord: optional.discord.unwrap_or(defaults.discord),
            minecraft: optional.minecraft,
        })
    }
}
