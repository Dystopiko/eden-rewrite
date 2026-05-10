use eden_minecraft_types::{BlockPos, GameType, ResourceKey};
use serde::{Deserialize, Serialize};
use twilight_model::id::{Id, marker::UserMarker};
use uuid::fmt::Hyphenated;

/// Request body of `POST /logs/commands`
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AlertCommand {
    pub command: String,
    pub executor: CommandExecutor,
}

/// Response body of `POST /logs/commands`
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandExecutor {
    Console,
    Player(PlayerExecutor),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlayerExecutor {
    pub dimension: ResourceKey,
    pub game_type: GameType,
    pub member_id: Option<Id<UserMarker>>,
    pub position: BlockPos,
    pub username: String,
    pub uuid: Hyphenated,
}

#[cfg(test)]
mod tests {
    use eden_minecraft_types::{BlockPos, Dimension, GameType};
    use insta::assert_json_snapshot;
    use twilight_model::id::Id;
    use uuid::Uuid;

    use crate::logs::commands::{AlertCommand, CommandExecutor, PlayerExecutor};

    #[test]
    fn test_serialization() {
        let _guard = crate::testing::setup(&["logs", "commands", "POST"]);

        let body: AlertCommand = AlertCommand {
            command: "/tell Notch I have secrets".into(),
            executor: CommandExecutor::Console,
        };
        assert_json_snapshot!("request-console", body);

        let body: AlertCommand = AlertCommand {
            command: "/tell Notch I have secrets".into(),
            executor: CommandExecutor::Player(PlayerExecutor {
                dimension: Dimension::OVERWORLD.resource_key().clone(),
                game_type: GameType::Creative,
                member_id: None,
                position: BlockPos::new(123, -10, -200),
                username: "steve".to_string(),
                uuid: Uuid::nil().hyphenated(),
            }),
        };
        assert_json_snapshot!("request-non-member-player", body);

        let body: AlertCommand = AlertCommand {
            command: "/tell Notch I have secrets".into(),
            executor: CommandExecutor::Player(PlayerExecutor {
                dimension: Dimension::OVERWORLD.resource_key().clone(),
                game_type: GameType::Creative,
                member_id: Some(Id::new(123456)),
                position: BlockPos::new(123, -10, -200),
                username: "steve".to_string(),
                uuid: Uuid::nil().hyphenated(),
            }),
        };
        assert_json_snapshot!("request-player", body);
    }
}
