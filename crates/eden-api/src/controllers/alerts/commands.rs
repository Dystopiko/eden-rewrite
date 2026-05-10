use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use eden_api_types::logs::commands::{AlertCommand, CommandExecutor};
use eden_jobs::alerts::AlertCommandJob;
use eden_model::alerts::command::{CommandAlert, CommandSource, PlayerInfo};
use eden_services::background_job_queue::BackgroundJobQueue;
use std::sync::Arc;

use crate::{context::WebContext, controllers::ApiResult};

pub async fn post(ctx: State<Arc<WebContext>>, body: Json<AlertCommand>) -> ApiResult<Response> {
    let job = AlertCommandJob(CommandAlert {
        command: body.command.clone(),
        source: match &body.executor {
            CommandExecutor::Console => CommandSource::Console,
            CommandExecutor::Player(inner) => CommandSource::Player(PlayerInfo {
                dimension: inner.dimension.clone().into(),
                game_type: inner.game_type.clone(),
                member_id: inner.member_id,
                position: inner.position,
                username: inner.username.clone(),
                uuid: inner.uuid.into_uuid(),
            }),
        },
    });

    BackgroundJobQueue::new(&ctx.pools).enqueue_job(job).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[cfg(test)]
mod tests {
    use eden_api_types::{
        eden_minecraft_types::{BlockPos, Dimension, GameType},
        logs::commands::{AlertCommand, CommandExecutor, PlayerExecutor},
    };
    use eden_model::alerts::command::{CommandSource, PlayerInfo};
    use eden_services::discord::MockDiscordService;
    use uuid::Uuid;

    use crate::testing::{TestApp, assert_response, setup_for_route};

    #[sqlx::test]
    async fn should_alert_command_use_from_console(pool: sqlx::PgPool) {
        let _guard = setup_for_route!["alerts", "commands", "POST"];

        let mut mock_discord_service = MockDiscordService::new();
        mock_discord_service
            .expect_log_command_by_admin()
            .once()
            .returning(|alert| {
                assert_eq!(alert.source, CommandSource::Console);
                Ok(())
            });

        let (app, server) = TestApp::builder(pool)
            .with_discord_service(mock_discord_service)
            .with_runner()
            .build();

        app.db_run_migrations().await;

        let response = server
            .post("/alerts/commands")
            .json(&AlertCommand {
                command: "/tp @p Notch".to_string(),
                executor: CommandExecutor::Console,
            })
            .await;

        assert_response!(response);

        // It should notify to the alerts channel
        app.run_pending_background_jobs().await.unwrap();
        app.assert_no_pending_jobs().await;
    }

    #[sqlx::test]
    async fn should_alert_command_use_from_player(pool: sqlx::PgPool) {
        let _guard = setup_for_route!["alerts", "commands", "POST"];

        let dimension = Dimension::OVERWORLD;
        let game_type = GameType::Survival;
        let position = BlockPos::new(123, -10, -12);
        let username = "steve".to_string();
        let uuid = Uuid::new_v4();

        let mut mock_discord_service = MockDiscordService::new();
        mock_discord_service
            .expect_log_command_by_admin()
            .once()
            .returning(move |alert| {
                let expected_source = CommandSource::Player(PlayerInfo {
                    dimension: Dimension::OVERWORLD,
                    game_type: GameType::Survival,
                    member_id: None,
                    position,
                    username: "steve".to_string(),
                    uuid,
                });
                assert_eq!(alert.source, expected_source);
                Ok(())
            });

        let (app, server) = TestApp::builder(pool)
            .with_discord_service(mock_discord_service)
            .with_runner()
            .build();

        app.db_run_migrations().await;

        let response = server
            .post("/alerts/commands")
            .json(&AlertCommand {
                command: "/tp @p Notch".to_string(),
                executor: CommandExecutor::Player(PlayerExecutor {
                    dimension: dimension.resource_key().clone(),
                    game_type,
                    member_id: None,
                    position,
                    username: username.clone(),
                    uuid: uuid.into(),
                }),
            })
            .await;

        assert_response!(response);

        // It should notify to the alerts channel
        app.run_pending_background_jobs().await.unwrap();
        app.assert_no_pending_jobs().await;
    }
}
