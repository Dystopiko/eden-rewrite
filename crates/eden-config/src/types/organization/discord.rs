use doku::Document;
use eden_config_derive::Validate;
use serde::Deserialize;
use std::collections::HashSet;
use twilight_model::id::{
    Id,
    marker::{GuildMarker, UserMarker},
};

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

    /// Configuration for the swearing police auto-response feature.
    #[serde(default)]
    pub swearing_police: SwearingPolice,
}

#[derive(Clone, Debug, Default, Deserialize, Document, Eq, PartialEq, Validate)]
#[serde(default)]
pub struct SwearingPolice {
    /// Additional words for the bot's profanity filter.
    /// Accepts words in any language that uses the Latin alphabet.
    #[doku(as = "Vec<String>")]
    #[validate(skip)]
    pub bad_words: HashSet<String>,

    /// A list of user IDs that are excluded from receiving warnings
    /// from the swearing police.
    #[doku(as = "Vec<String>")]
    #[validate(skip)]
    pub excluded_users: HashSet<Id<UserMarker>>,

    /// Extra warning message templates the swearing police can choose
    /// from in random, in addition to the built-in defaults.
    ///
    /// **Placeholders**:
    /// - `{BAD_WORDS}` - The bad words detected in the message
    /// - `{LINKING_VERB}` - Linking verb (is/are) matching the number of bad words
    /// - `{PREFERRED_USER_NAME}` - The user's preferred name, resolved in this order:
    ///
    /// ```txt
    /// guild nickname -> global display name -> Discord username
    /// ```
    #[validate(skip)]
    pub warning_templates: Vec<String>,
}
