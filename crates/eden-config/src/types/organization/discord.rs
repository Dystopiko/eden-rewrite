use doku::Document;
use eden_config_derive::Validate;
use serde::Deserialize;
use twilight_model::id::{Id, marker::GuildMarker};

use crate::types::Token;

#[derive(Clone, Debug, Deserialize, Document, Eq, PartialEq, Validate)]
pub struct Discord {
    /// Discord bot authorization token.
    ///
    /// This is the authentication token for your Discord bot, obtained from
    /// the Discord Developer Portal. It must be kept secure and never shared
    /// publicly.
    ///
    /// Get your token at: https://discord.com/developers/applications
    #[doku(as = "String", example = "<INSERT BOT TOKEN HERE>")]
    #[validate(with = "super::validators::validate_discord_token")]
    pub token: Token,

    /// The Discord guild (server) ID related to the organization.
    ///
    /// This is the unique identifier for the organization's Discord server.
    /// You can find this by enabling Developer Mode in Discord settings,
    /// then right-clicking on your server and selecting "Copy Server ID".
    ///
    /// The ID must be a valid Discord snowflake (a 64-bit integer).
    #[doku(as = "String", example = "1")]
    #[validate(skip)]
    pub guild_id: Id<GuildMarker>,
}
