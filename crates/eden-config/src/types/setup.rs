//! Initial setup configuration for new guilds.

use crate::validation::Validate;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Validate)]
#[serde(default)]
pub struct Setup {
    pub settings: InitialSettings,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Validate)]
pub struct InitialSettings {
    /// Whether to allow guests to join the Minecraft server.
    #[validate(skip)]
    pub allow_guests: bool,
}

impl Default for InitialSettings {
    fn default() -> Self {
        Self { allow_guests: true }
    }
}
