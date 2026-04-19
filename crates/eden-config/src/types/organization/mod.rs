use doku::Document;
use eden_config_derive::Validate;
use serde::Deserialize;

mod discord;
mod minecraft;
mod validators;

pub use self::discord::Discord;
pub use self::minecraft::Minecraft;

#[derive(Clone, Debug, Deserialize, Document, Eq, PartialEq, Validate)]
#[serde(default)]
pub struct Organization {
    /// The name of the organization.
    ///
    /// This field is required as Eden will customize all of its messages
    /// catered to your organization.
    ///
    /// If not specified, the default value is `Dystopia`.
    #[validate(skip)]
    pub name: String,

    /// Discord configuration related to the organization's Discord guild (server).
    ///
    /// If this table is missing, Discord bot service will be disabled
    /// automatically and any incoming messages will not be processed
    /// internally.
    pub discord: Option<Discord>,

    /// Minecraft server management configuration.
    pub minecraft: Minecraft,
}

impl Default for Organization {
    fn default() -> Self {
        Self {
            name: "Dystopia".to_string(),
            discord: None,
            minecraft: Minecraft::default(),
        }
    }
}
