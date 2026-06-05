use axum::http::StatusCode;
use eden_api_server::{ApiError, error::ErrorCode};
use eden_api_types::eden_minecraft_types::McEdition;
use eden_common::domain::{cache::MockCache, discord::MockDiscordClient, notifier::MockNotifier};
use eden_config::types::organization::minecraft::PerkId;
use eden_model::{
    common::ApprovalStatus,
    tables::{
        linked_mc_account::LinkMcAccount,
        mc_login_event::McLoginEvent,
        tokens::{PermissionScope, TokenType},
    },
};
use insta::assert_snapshot;
use serde_json::json;
use std::{net::IpAddr, str::FromStr};
use twilight_model::id::Id;
use uuid::Uuid;

use crate::harness::{OrganizationSetup, TestHarness};

#[sqlx::test]
async fn test_incompatible_account_edition(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).with_runner().build();
    harness.run_migrations().await;

    let ip = IpAddr::from_str("127.0.0.1").unwrap();
    let member_id = Id::new(12345);
    let status = ApprovalStatus::Approved;

    harness.db_new_member(member_id, "steve").await;
    harness.db_new_cidr_trust(member_id, ip, status).await;

    let mut conn = harness.db_conn().await;
    let account = LinkMcAccount::builder()
        .edition(McEdition::Java)
        .member_id(member_id)
        .username("steve")
        .uuid(Uuid::new_v4())
        .build()
        .insert(&mut conn)
        .await
        .unwrap();

    drop(conn);

    let user = harness.as_minecraft_server().await;
    let response = user
        .post("/minecraft/login")
        .json(&json!({
            "uuid": account.uuid,
            "ip": ip,
            "edition": McEdition::Bedrock,
        }))
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
    response.assert_json_contains(&json!({
        "message": "Incompatible edition",
    }));

    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_rejected_ip(pool: sqlx::PgPool) {
    let ip = IpAddr::from_str("127.0.0.1").unwrap();
    let member_id = Id::new(12345);
    let status = ApprovalStatus::Revoked;

    let mut notifier = MockNotifier::new();
    notifier.expect_revoked_login().return_once(move |login| {
        assert_eq!(login.ip, ip);
        assert_eq!(login.member_id, member_id);
        Ok(())
    });

    let harness = TestHarness::builder(pool)
        .with_notifier(notifier)
        .with_runner()
        .build();

    harness.run_migrations().await;

    harness.db_new_member(member_id, "steve").await;
    harness.db_new_cidr_trust(member_id, ip, status).await;

    let mut conn = harness.db_conn().await;
    let account = LinkMcAccount::builder()
        .edition(McEdition::Java)
        .member_id(member_id)
        .username("steve")
        .uuid(Uuid::new_v4())
        .build()
        .insert(&mut conn)
        .await
        .unwrap();

    drop(conn);

    let user = harness.as_minecraft_server().await;
    let response = user
        .post("/minecraft/login")
        .json(&json!({
            "uuid": account.uuid,
            "ip": ip,
            "edition": McEdition::Java,
        }))
        .await;

    response.assert_status(StatusCode::FORBIDDEN);
    assert_snapshot!(response.text());

    harness.run_pending_jobs().await.unwrap();
    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_unknown_ip(pool: sqlx::PgPool) {
    let ip = IpAddr::from_str("127.0.0.1").unwrap();
    let member_id = Id::new(12345);

    let mut discord = MockDiscordClient::new();
    discord
        .expect_notify_pending_login()
        .return_once(move |login| {
            assert_eq!(login.member_id, member_id);
            assert_eq!(login.ip, ip);
            Ok(())
        });

    let harness = TestHarness::builder(pool)
        .with_discord_client(discord)
        .with_runner()
        .build();

    harness.run_migrations().await;
    harness.db_new_member(member_id, "steve").await;

    let mut conn = harness.db_conn().await;
    let account = LinkMcAccount::builder()
        .edition(McEdition::Java)
        .member_id(member_id)
        .username("steve")
        .uuid(Uuid::new_v4())
        .build()
        .insert(&mut conn)
        .await
        .unwrap();

    drop(conn);

    let user = harness.as_minecraft_server().await;
    let response = user
        .post("/minecraft/login")
        .json(&json!({
            "uuid": account.uuid,
            "ip": ip,
            "edition": McEdition::Java,
        }))
        .await;

    response
        .assert_status(StatusCode::FORBIDDEN)
        .assert_json_contains(&json!({
            "code": ErrorCode::Forbidden.to_string(),
        }));

    harness.run_pending_jobs().await.unwrap();
    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_usage_with_user_tokens(pool: sqlx::PgPool) {
    let mut cache = MockCache::new();
    cache.expect_find_token().return_once(|_| Ok(None));
    cache.expect_update_token().return_once(|_, token| {
        assert_eq!(token.kind, TokenType::User);
        Ok(())
    });

    let harness = TestHarness::builder(pool).with_cache(cache).build();
    harness.run_migrations().await;

    let ip = IpAddr::from_str("127.0.0.1").unwrap();
    let member_id = Id::new(12345);

    harness.db_new_member(member_id, "steve").await;
    harness.db_new_staff(member_id).await;

    let user = harness
        .as_member_user(member_id, PermissionScope::empty())
        .await;

    let response = user
        .post("/minecraft/login")
        .json(&json!({
            "uuid": Uuid::new_v4(),
            "ip": ip,
            "edition": McEdition::Java
        }))
        .await;

    response.assert_status(StatusCode::UNAUTHORIZED);
    response.assert_json(&ApiError::ACCESS_DENIED.serialize());

    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_disabled_guest_access(pool: sqlx::PgPool) {
    let mut cache = MockCache::new();
    cache.expect_find_settings().return_once(|guild_id| {
        assert_eq!(guild_id, Id::new(12345));
        Ok(None)
    });
    cache.expect_update_settings().return_once(|settings| {
        assert!(!settings.allow_guests);
        Ok(())
    });
    cache
        .expect_find_linked_mc_account()
        .return_once(|_| Ok(None));

    let harness = TestHarness::builder(pool)
        .with_config_str(
            r#"
            [organization.discord]
            guild_id = "12345"
            token = "123"
            
            [setup.settings]
            allow_guests = false"#,
        )
        .with_discord_client(MockDiscordClient::new())
        .build();

    harness.run_migrations().await;

    let ip = IpAddr::from_str("127.0.0.1").unwrap();
    let user = harness.as_minecraft_server().await;

    let response = user
        .post("/minecraft/login")
        .json(&json!({
            "uuid": Uuid::new_v4(),
            "ip": ip,
            "edition": McEdition::Java,
        }))
        .await;

    response
        .assert_status(StatusCode::LOCKED)
        .assert_json_contains(&json!({
            "code": ErrorCode::GuestAccessDisabled.to_string(),
        }));

    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_grant_session_as_guest(pool: sqlx::PgPool) {
    let mut cache = MockCache::new();
    let mut notifier = MockNotifier::new();

    cache.expect_find_settings().return_once(|guild_id| {
        assert_eq!(guild_id, Id::new(12345));
        Ok(None)
    });
    cache
        .expect_find_linked_mc_account()
        .return_once(|_| Ok(None));

    notifier
        .expect_guest_player_joined()
        .return_once(|_| Ok(()));

    let harness = TestHarness::builder(pool)
        .with_config_str(
            r#"
            [organization.discord]
            guild_id = "12345"
            token = "123""#,
        )
        .with_discord_client(MockDiscordClient::new())
        .with_notifier(notifier)
        .with_runner()
        .build();

    harness.run_migrations().await;

    let ip = IpAddr::from_str("127.0.0.1").unwrap();
    let user = harness.as_minecraft_server().await;

    let response = user
        .post("/minecraft/login")
        .json(&json!({
            "uuid": Uuid::new_v4(),
            "ip": ip,
            "edition": McEdition::Java,
        }))
        .await;

    response.assert_status(StatusCode::OK);
    assert_snapshot!(response.text());

    harness.run_pending_jobs().await.unwrap();
    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_grant_session_as_member(pool: sqlx::PgPool) {
    let org = OrganizationSetup::builder()
        .discord_guild_id(Id::new(123456))
        .perks(PerkId::Members, &["veinminer.use"])
        .build();

    let harness = TestHarness::builder(pool)
        .with_discord_client(MockDiscordClient::new())
        .with_organization(org)
        .with_runner()
        .build();

    harness.run_migrations().await;

    let ip = IpAddr::from_str("127.0.0.1").unwrap();
    let member_id = Id::new(12345);
    let status = ApprovalStatus::Approved;

    harness.db_new_member(member_id, "steve").await;
    harness.db_new_cidr_trust(member_id, ip, status).await;

    let mut conn = harness.db_conn().await;
    let account = LinkMcAccount::builder()
        .edition(McEdition::Java)
        .member_id(member_id)
        .username("steve")
        .uuid(Uuid::new_v4())
        .build()
        .insert(&mut conn)
        .await
        .unwrap();

    drop(conn);

    let user = harness.as_minecraft_server().await;
    let response = user
        .post("/minecraft/login")
        .json(&json!({
            "uuid": account.uuid,
            "ip": ip,
            "edition": McEdition::Java,
        }))
        .await;

    response.assert_status(StatusCode::OK);
    response.assert_json_contains(&json!({
        "perks": ["veinminer.use"],
    }));
    assert_snapshot!(response.text());

    harness.run_pending_jobs().await.unwrap();
    harness.assert_no_pending_jobs().await;

    // It should log successful login
    let mut conn = harness.db_conn().await;
    let events = sqlx::query_as::<_, McLoginEvent>("SELECT * FROM mc_login_events")
        .fetch_all(&mut *conn)
        .await
        .expect("could not fetch all login events");

    assert!(!events.is_empty(), "should log successful login");
}
