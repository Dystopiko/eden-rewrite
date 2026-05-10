use eden_minecraft_types::{BlockPos, Dimension, GameType};
use serde::{Deserialize, Serialize};
use twilight_model::id::{Id, marker::UserMarker};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandAlert {
    pub command: String,
    pub source: CommandSource,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandSource {
    Console,
    Player(PlayerInfo),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlayerInfo {
    pub dimension: Dimension,
    pub game_type: GameType,
    pub member_id: Option<Id<UserMarker>>,
    pub position: BlockPos,
    pub username: String,
    pub uuid: Uuid,
}
