//! Initial setup configuration for new guilds.

use crate::validation::Validate;
use doku::Document;
use serde::Deserialize;

/// Top-level setup configuration containing initial deployment settings.
///
/// This structure groups all default configuration values that should be
/// applied when Eden is first deployed to a new guild or when performing
/// a fresh installation.
#[derive(Clone, Debug, Default, Deserialize, Document, Eq, PartialEq, Validate)]
#[serde(default)]
pub struct Setup {
    /// Initial settings to apply upon first deployment.
    ///
    /// These settings control the default behavior of the bot in newly
    /// configured guilds.
    pub settings: InitialSettings,
}

/// Initial settings applied to new guild deployments.
///
/// These settings define the default behavior and access controls for
/// guilds where Eden has just been installed or reset to defaults.
#[derive(Clone, Debug, Deserialize, Document, Eq, PartialEq, Validate)]
pub struct InitialSettings {
    /// Whether to allow guests to join the Minecraft server.
    ///
    /// When `true`, users without specific roles can connect to the
    /// Minecraft server. When `false`, only users with designated roles
    /// can connect.
    ///
    /// Defaults to `true` for open access.
    #[validate(skip)]
    pub allow_guests: bool,
}

impl Default for InitialSettings {
    fn default() -> Self {
        Self { allow_guests: true }
    }
}
