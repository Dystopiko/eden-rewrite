use crate::validation::Validate;
use doku::Document;
use serde::Deserialize;
use twilight_model::id::{Id, marker::GuildMarker};

/// Primary guild configuration.
///
/// Specifies the Discord guild (server) where the bot operates as its
/// primary or home server. This is typically where administrative commands
/// and core features are available.
#[derive(Clone, Debug, Deserialize, Document, Eq, PartialEq, Validate)]
pub struct PrimaryGuild {
    /// The Discord guild (server) ID.
    ///
    /// This is the unique identifier for your Discord server. You can find
    /// this by enabling Developer Mode in Discord settings, then right-clicking
    /// on your server and selecting "Copy ID".
    ///
    /// The ID must be a valid Discord snowflake (a 64-bit integer).
    #[doku(as = "String", example = "1")]
    #[validate(skip)]
    pub id: Id<GuildMarker>,
}
