use doku::Document;
use eden_config_derive::Validate;
use serde::Deserialize;
use twilight_model::id::{Id, marker::GuildMarker};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Document, Eq, PartialEq, Validate)]
#[serde(default)]
pub struct Minecraft {
    /// These can be activated for contributors, members, and other designated
    /// players by setting up a list of supported permissions, allowing the Minecraft
    /// server client to utilize the LuckPerms API to apply the appropriate changes.
    ///
    /// Define LuckPerms permission nodes to be automatically granted to players
    /// based on their role. Supports role-based and player-specific targeting.
    ///
    /// ```toml
    /// contributors = ["veinminer"]
    /// members = ["dystopia.instantrestock"]
    /// staff = []
    /// admins = []
    ///
    /// # Player-specific examples (by their Discord ID or UUID):
    /// "745809834183753828" = ["deadchest"]
    /// "a1705729-8729-3a49-befe-0ee68d88a374" = ["veinminer"]
    /// ```
    pub perks: Perks,
}

#[derive(Clone, Debug, Default, Deserialize, Document, Eq, PartialEq, Hash, Validate)]
pub struct Perks {
    #[validate(skip)]
    pub contributors: Vec<String>,
    #[validate(skip)]
    pub members: Vec<String>,
    #[validate(skip)]
    pub staff: Vec<String>,
    #[validate(skip)]
    pub admins: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum PlayerIdentifier {
    Discord(Id<GuildMarker>),
    Uuid(Uuid),
}
