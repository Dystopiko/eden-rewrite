mod primary_guild;
mod token;
mod validators;

pub use primary_guild::PrimaryGuild;
pub use token::Token;

use crate::validation::Validate;
use serde::Deserialize;

/// This section contains all configuration related to the Discord bot
/// itself, such as authentication tokens and primary guild settings.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Validate)]
pub struct Bot {
    /// Configuration for the primary guild where Eden operates
    pub primary_guild: Option<PrimaryGuild>,

    /// Discord bot authorization token
    #[validate(with = "self::validators::validate_token")]
    pub token: Token,
}
