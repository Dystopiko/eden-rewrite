use axum::http::StatusCode;
use eden_api_server::ApiError;
use eden_api_types::{
    eden_minecraft_types::McEdition, error::Error, members::link::LinkMcAccountChallenge,
};
use eden_common::{challenge_code::RawChallengeCode, domain::cache::MockCache};
use eden_model::tables::{
    linked_mc_account::LinkMcAccount,
    mc_account_link_challenge::NewMcLinkChallenge,
    tokens::{PermissionScope, TokenType},
};
use insta::assert_snapshot;
use serde_json::json;
use std::{net::IpAddr, str::FromStr, time::Duration};
use twilight_model::id::Id;
use uuid::Uuid;

use crate::harness::TestHarness;

#[sqlx::test]
async fn test_for_java_accounts(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).build();
    harness.run_migrations().await;

    let ip = IpAddr::from_str("127.0.0.1").unwrap();
    let uuid = Uuid::new_v4();

    let user = harness.as_minecraft_server().await;
    let response = user
        .post("/minecraft/link")
        .json(&json!({
            "uuid": uuid,
            "username": "steve",
            "ip": ip,
            "edition": McEdition::Java,
        }))
        .await;

    response.assert_status(StatusCode::ACCEPTED);

    let body = response.json::<LinkMcAccountChallenge>();
    let code = RawChallengeCode::parse(&body.code);
    assert!(code.is_some());

    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_for_bedrock_accounts(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).build();
    harness.run_migrations().await;

    let ip = IpAddr::from_str("127.0.0.1").unwrap();
    let uuid = Uuid::new_v4();

    let user = harness.as_minecraft_server().await;
    let response = user
        .post("/minecraft/link")
        .json(&json!({
            "uuid": uuid,
            "username": "steve",
            "ip": ip,
            "edition": McEdition::Bedrock,
        }))
        .await;

    response.assert_status(StatusCode::ACCEPTED);

    let body = response.json::<LinkMcAccountChallenge>();
    let code = RawChallengeCode::parse(&body.code);
    assert!(code.is_some());

    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_pending_link_challenge(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).build();
    harness.run_migrations().await;

    let ip = IpAddr::from_str("127.0.0.1").unwrap();
    let uuid = Uuid::new_v4();

    let mut conn = harness.db_tx().await;
    NewMcLinkChallenge::builder()
        .username("steve")
        .player_uuid(uuid)
        .ttl(Duration::from_secs(30))
        .ip_address(ip)
        .hashed_code("invalid")
        .edition(McEdition::Java)
        .build()
        .insert(&mut conn)
        .await
        .unwrap();

    conn.commit().await.expect("failed to commit transaction");

    let user = harness.as_minecraft_server().await;
    let response = user
        .post("/minecraft/link")
        .json(&json!({
            "uuid": uuid,
            "username": "steve",
            "ip": ip,
            "edition": McEdition::Bedrock,
        }))
        .await;

    response.assert_status(StatusCode::CONFLICT);

    let error = response.json::<Error>();
    assert!(error.message.contains("pending link challenge"));
    assert_snapshot!(response.text());

    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_already_linked_mc_account(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).build();
    harness.run_migrations().await;

    let ip = IpAddr::from_str("127.0.0.1").unwrap();
    let member_id = Id::new(12345);
    let uuid = Uuid::new_v4();

    harness.db_new_member(member_id, "steve").await;

    let mut conn = harness.db_conn().await;
    LinkMcAccount::builder()
        .edition(McEdition::Java)
        .member_id(member_id)
        .username("steve")
        .uuid(uuid)
        .build()
        .insert(&mut conn)
        .await
        .unwrap();

    drop(conn);

    let user = harness.as_minecraft_server().await;
    let response = user
        .post("/minecraft/link")
        .json(&json!({
            "uuid": uuid,
            "username": "steve",
            "ip": ip,
            "edition": McEdition::Bedrock,
        }))
        .await;

    response.assert_status(StatusCode::CONFLICT);
    response.assert_json_contains(&json!({
        "message": "This Minecraft account is already linked to a Discord account.",
    }));

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
        .post("/minecraft/link")
        .json(&json!({
            "uuid": Uuid::new_v4(),
            "username": "steve",
            "ip": ip,
            "edition": McEdition::Java,
        }))
        .await;

    response.assert_status(StatusCode::UNAUTHORIZED);
    response.assert_json(&ApiError::ACCESS_DENIED.serialize());

    harness.assert_no_pending_jobs().await;
}
