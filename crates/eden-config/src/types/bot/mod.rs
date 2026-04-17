mod primary_guild;
mod token;
mod validators;

pub use primary_guild::PrimaryGuild;
pub use token::Token;

use crate::validation::Validate;
use doku::Document;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, Document, Eq, PartialEq, Validate)]
pub struct Bot {
    /// Configuration for the primary Discord guild (server).
    ///
    /// The primary guild is the main server where the bot operates and
    /// manages its core functionality. This is optional and can be omitted
    /// if the bot operates across multiple guilds without a primary focus.
    pub primary_guild: Option<PrimaryGuild>,

    /// Discord bot authorization token.
    ///
    /// This is the authentication token for your Discord bot, obtained from
    /// the Discord Developer Portal. It must be kept secure and never shared
    /// publicly.
    ///
    /// Get your token at: https://discord.com/developers/applications
    #[doku(as = "String", example = "<INSERT BOT TOKEN HERE>")]
    #[validate(with = "self::validators::validate_token")]
    pub token: Token,
}
