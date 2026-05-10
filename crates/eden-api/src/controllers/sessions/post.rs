use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use eden_api_types::{
    error::ErrorCode,
    sessions::{RequestSession, SessionGranted},
    types::{EncodedMember, MinimalMemberStatus},
};
use eden_jobs::{events::OnPlayerJoinedJob, notification::pending_ip_login::NotifyPendingIpLogin};
use eden_model::{
    common::ApprovalStatus,
    tables::{
        linked_mc_account_view::LinkedMcAccountView, mc_login_event::NewMcLoginEvent,
        member_cidr_trust::MemberCidrTrust, settings::Settings,
    },
};
use eden_postgres::error::QueryResultExt;
use eden_services::{
    background_job_queue::BackgroundJobQueue,
    ext::{LinkedMcAccountViewExt, MemberFlagsExt},
    minecraft::resolve_perks,
    repository::CachedRepository,
};
use std::sync::Arc;

use crate::{context::WebContext, controllers::ApiResult, error::ApiError};

pub async fn post(ctx: State<Arc<WebContext>>, body: Json<RequestSession>) -> ApiResult<Response> {
    let repository = CachedRepository::new(&*ctx.cache, &ctx.pools);

    let Some(account) = repository
        .find_linked_account_view(body.uuid.into_uuid())
        .await
        .optional()?
    else {
        let settings = repository.settings(&ctx.config.get()).await?;
        check_if_guest_entry_is_allowed(&settings).await?;

        // Returns as guest session
        let body = Json(SessionGranted {
            member: None,
            perks: Vec::new(),
        });

        return Ok((StatusCode::CREATED, body).into_response());
    };

    validate_cidr_trust(&ctx, &repository, &account, &body).await?;
    validate_edition(&account, &body)?;

    let perks = resolve_perks(
        &ctx.config.get().organization.minecraft,
        account.flags,
        Some(account.discord_user_id.cast()),
        Some(account.uuid),
    );

    let response = build_response_as_member(&account, perks);
    enqueue_login_event(&ctx, &account, &body).await;

    Ok((StatusCode::CREATED, Json(response)).into_response())
}

async fn enqueue_login_event(
    ctx: &WebContext,
    account: &LinkedMcAccountView,
    body: &RequestSession,
) {
    let event = NewMcLoginEvent::from_linked(&account.simplify())
        .ip_address(body.ip)
        .build();

    if let Err(error) = BackgroundJobQueue::new(&ctx.pools)
        .enqueue_job(OnPlayerJoinedJob(event))
        .await
    {
        tracing::warn!(?error, "failed to enqueue OnPlayerJoinedJob job");
    }
}

async fn notify_pending_ip_login(ctx: &WebContext, cidr_trust: &MemberCidrTrust) {
    let job = NotifyPendingIpLogin(cidr_trust.clone());
    if let Err(error) = BackgroundJobQueue::new(&ctx.pools).enqueue_job(job).await {
        tracing::warn!(
            ?error,
            "failed to notify the member about unrecognized ip login"
        );
    }
}

async fn check_if_guest_entry_is_allowed(settings: &Settings) -> ApiResult<()> {
    if !settings.allow_guests {
        return Err(ApiError::from_static(
            ErrorCode::InvalidRequest,
            "Guest access is disabled by an adminstrator",
        ));
    }

    Ok(())
}

async fn validate_cidr_trust(
    ctx: &WebContext,
    repository: &CachedRepository<'_>,
    account: &LinkedMcAccountView,
    body: &RequestSession,
) -> ApiResult<()> {
    let (cidr_trust, can_notify_to_member) = repository
        .resolve_member_cidr_trust(account.member.discord_user_id.cast(), body.ip)
        .await?;

    match cidr_trust.status {
        ApprovalStatus::Approved => Ok(()),
        ApprovalStatus::Pending => {
            if can_notify_to_member {
                notify_pending_ip_login(ctx, &cidr_trust).await;
            }

            Err(ApiError::from_static(
                ErrorCode::InvalidRequest,
                "Unrecognized IP address detected. Check your Eden notifications to approve \
                or block this login attempt.",
            ))
        }
        ApprovalStatus::Revoked => Err(ApiError::from_static(
            ErrorCode::InvalidRequest,
            "Your IP address has been blocked from accessing this account. \
            Please contact support if you believe this is a mistake.",
        )),
    }
}

fn validate_edition(account: &LinkedMcAccountView, body: &RequestSession) -> ApiResult<()> {
    if account.edition != body.edition {
        return Err(ApiError::from_static(
            ErrorCode::InvalidRequest,
            "Incompatible account type",
        ));
    }
    Ok(())
}

fn build_response_as_member(account: &LinkedMcAccountView, perks: Vec<String>) -> SessionGranted {
    SessionGranted {
        member: Some(EncodedMember {
            id: account.discord_user_id.cast(),
            name: account.name.clone(),
            status: Some(MinimalMemberStatus::Okay),
            last_login_at: None,
            rank: Some(account.flags.api_name().to_string()),
        }),
        perks,
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use eden_api_types::{
        eden_minecraft_types::McEdition, eden_timestamp::Timestamp, sessions::RequestSession,
    };
    use eden_config::{Config, types::setup::InitialSettings};
    use eden_model::{
        common::ApprovalStatus,
        snowflake::Snowflake,
        tables::{
            linked_mc_account_view::LinkedMcAccountView,
            member_cidr_trust::MemberCidrTrust,
            member_view::{MemberFlags, MemberView},
        },
    };
    use eden_services::{cache::MockCache, discord::MockDiscordService};
    use sqlx::types::ipnet::IpNet;
    use std::{net::IpAddr, path::Path, str::FromStr};
    use twilight_model::id::{Id, marker::UserMarker};
    use uuid::Uuid;

    use crate::testing::{TestApp, assert_response, setup_for_route};

    #[sqlx::test]
    async fn should_reject_if_guest_access_is_disabled(pool: sqlx::PgPool) {
        let _guard = setup_for_route!["sessions", "POST"];

        let (app, server) = TestApp::builder(pool)
            .with_discord_service(MockDiscordService::new())
            .with_runner()
            .build();

        app.db_run_migrations().await;
        app.db_set_settings(InitialSettings {
            allow_guests: false,
            ..Default::default()
        })
        .await;

        let ip = IpAddr::from_str("127.0.0.1").unwrap();
        let uuid = Uuid::new_v4();

        let response = server
            .post("/sessions")
            .json(&RequestSession {
                uuid: uuid.hyphenated(),
                ip,
                edition: McEdition::Java,
            })
            .await;

        response.assert_status(StatusCode::BAD_REQUEST);
        assert_response!(response as str);
    }

    #[sqlx::test]
    async fn should_provide_perks_for_other_member_roles(pool: sqlx::PgPool) {
        let _guard = setup_for_route!["sessions", "POST"];

        let toml = r#"
        [organization.minecraft.perks]
        contributors = ["veinminer"]
        "#;

        let (config, _) = Config::maybe_toml_file(toml, Path::new("eden.toml")).unwrap();
        let (app, server) = TestApp::builder(pool)
            .with_config(config)
            .with_discord_service(MockDiscordService::new())
            .with_runner()
            .build();

        app.db_run_migrations().await;

        let discord_id = Id::new(1);
        let ip = IpAddr::from_str("127.0.0.1").unwrap();
        let uuid = Uuid::new_v4();

        app.db_new_contributor(discord_id, "steve").await;
        app.db_trust_ip(discord_id, ip).await;
        app.db_link_mc_account(discord_id, uuid, "steve", McEdition::Java)
            .await;

        let response = server
            .post("/sessions")
            .json(&RequestSession {
                uuid: uuid.hyphenated(),
                ip: IpAddr::from_str("127.0.0.1").unwrap(),
                edition: McEdition::Java,
            })
            .await;

        response.assert_status(StatusCode::CREATED);
        assert_response!(response as str);

        app.run_pending_background_jobs().await.unwrap();
        app.assert_no_pending_jobs().await;
    }

    #[sqlx::test]
    async fn should_utilize_cached_resources(pool: sqlx::PgPool) {
        let _guard = setup_for_route!["sessions", "POST"];

        let discord_user_id: Id<UserMarker> = Id::new(1);
        let ip = IpAddr::from_str("127.0.0.1").unwrap();
        let uuid = Uuid::new_v4();

        let cached_view = LinkedMcAccountView {
            member: MemberView {
                discord_user_id: Snowflake::new(discord_user_id.cast()),
                joined_at: Timestamp::now(),
                name: "steve".to_string(),
                flags: MemberFlags::REGULAR,
                inviter: None,
            },
            uuid,
            linked_at: Timestamp::now(),
            username: "steve".to_string(),
            edition: McEdition::Java,
        };

        let cached_cidr_trust: MemberCidrTrust = MemberCidrTrust {
            id: Uuid::new_v4(),
            member_id: Snowflake::new(discord_user_id.cast()),
            cidr: IpNet::from_str("127.0.0.1/32").unwrap(),
            created_at: Timestamp::now(),
            status: ApprovalStatus::Approved,
            updated_at: None,
        };

        let mut mock_cache = MockCache::new();
        mock_cache
            .expect_find_linked_account_view()
            .return_once(|_| Ok(Some(cached_view)));

        mock_cache
            .expect_find_member_cidr_trust_entry()
            .return_once(|_, _| Ok(Some(cached_cidr_trust)));

        let (app, server) = TestApp::builder(pool)
            .with_cache(mock_cache)
            .with_discord_service(MockDiscordService::new())
            .with_runner()
            .build();

        let response = server
            .post("/sessions")
            .json(&RequestSession {
                uuid: uuid.hyphenated(),
                ip,
                edition: McEdition::Java,
            })
            .await;

        response.assert_status(StatusCode::CREATED);
        assert_response!(response as str);

        app.db_run_migrations().await;
    }

    #[sqlx::test]
    async fn should_notify_guest_logged_in(pool: sqlx::PgPool) {
        let _guard = setup_for_route!["sessions", "POST"];
        let (app, server) = TestApp::builder(pool)
            .with_discord_service(MockDiscordService::new())
            .with_runner()
            .build();

        app.db_run_migrations().await;

        let ip = IpAddr::from_str("127.0.0.1").unwrap();
        let uuid = Uuid::new_v4();

        let response = server
            .post("/sessions")
            .json(&RequestSession {
                uuid: uuid.hyphenated(),
                ip,
                edition: McEdition::Java,
            })
            .await;

        response.assert_status(StatusCode::CREATED);
        assert_response!(response as str);

        // It should notify to the user about this
        app.run_pending_background_jobs().await.unwrap();
        app.assert_no_pending_jobs().await;
    }

    #[sqlx::test]
    async fn should_notify_untrusted_ips(pool: sqlx::PgPool) {
        let _guard = setup_for_route!["sessions", "POST"];
        let (app, server) = TestApp::builder(pool)
            .with_discord_service(MockDiscordService::new())
            .with_runner()
            .build();

        app.db_run_migrations().await;

        let discord_id = Id::new(1);
        let ip = IpAddr::from_str("127.0.0.1").unwrap();
        let uuid = Uuid::new_v4();

        app.db_new_member(discord_id, "steve").await;
        app.db_link_mc_account(discord_id, uuid, "steve", McEdition::Java)
            .await;

        let response = server
            .post("/sessions")
            .json(&RequestSession {
                uuid: uuid.hyphenated(),
                ip,
                edition: McEdition::Java,
            })
            .await;

        response.assert_status(StatusCode::BAD_REQUEST);
        assert_response!(response as str);

        // It should notify to the user about this
        app.run_pending_background_jobs().await.unwrap();
        app.assert_no_pending_jobs().await;
    }

    #[sqlx::test]
    async fn should_reject_untrusted_ips(pool: sqlx::PgPool) {
        let _guard = setup_for_route!["sessions", "POST"];

        let (app, server) = TestApp::builder(pool).build();
        app.db_run_migrations().await;

        let discord_id = Id::new(1);
        let ip = IpAddr::from_str("127.0.0.1").unwrap();
        let uuid = Uuid::new_v4();

        app.db_new_member(discord_id, "steve").await;
        app.db_revoke_ip(discord_id, ip).await;
        app.db_link_mc_account(discord_id, uuid, "steve", McEdition::Java)
            .await;

        let response = server
            .post("/sessions")
            .json(&RequestSession {
                uuid: uuid.hyphenated(),
                ip,
                edition: McEdition::Java,
            })
            .await;

        response.assert_status(StatusCode::BAD_REQUEST);
        assert_response!(response as str);

        app.assert_no_pending_jobs().await;
    }

    #[sqlx::test]
    async fn should_grant_session_as_regular_member(pool: sqlx::PgPool) {
        let _guard = setup_for_route!["sessions", "POST"];
        let (app, server) = TestApp::builder(pool)
            .with_discord_service(MockDiscordService::new())
            .with_runner()
            .build();

        app.db_run_migrations().await;

        let discord_id = Id::new(1);
        let ip = IpAddr::from_str("127.0.0.1").unwrap();
        let uuid = Uuid::new_v4();

        app.db_new_member(discord_id, "steve").await;
        app.db_trust_ip(discord_id, ip).await;
        app.db_link_mc_account(discord_id, uuid, "steve", McEdition::Java)
            .await;

        let response = server
            .post("/sessions")
            .json(&RequestSession {
                uuid: uuid.hyphenated(),
                ip: IpAddr::from_str("127.0.0.1").unwrap(),
                edition: McEdition::Java,
            })
            .await;

        response.assert_status(StatusCode::CREATED);
        assert_response!(response as str);

        app.run_pending_background_jobs().await.unwrap();
        app.assert_no_pending_jobs().await;
    }
}
