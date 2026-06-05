use axum::http::StatusCode;
use eden_api_types::{admin::settings::EncodedSettings, eden_timestamp::Timestamp};
use eden_model::tables::tokens::PermissionScope;
use insta::assert_json_snapshot;
use serde_json::json;
use twilight_model::id::Id;

use crate::harness::TestHarness;

const SETTINGS: &str = r#"
[organization.discord]
guild_id = "123456"
token = "discord.token"
"#;

#[sqlx::test]
async fn test_patch_route(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).with_config_str(SETTINGS).build();
    harness.run_migrations().await;

    let member_id = Id::new(123456);
    harness.db_new_member(member_id, "steve").await;
    harness.db_new_admin_staff(member_id).await;

    let user = harness
        .as_member_user(member_id, PermissionScope::EDIT_SETTINGS)
        .await;

    let response = user
        .patch("/admin/settings")
        .json(&json!({
            "allow_guests": false,
        }))
        .await;

    response.assert_status(StatusCode::OK);

    let mut body = response.json::<EncodedSettings>();
    body.updated_at = Timestamp::from_secs(0).unwrap();

    assert_json_snapshot!(body);
    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_get_route(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).with_config_str(SETTINGS).build();
    harness.run_migrations().await;

    let member_id = Id::new(123456);
    harness.db_new_member(member_id, "steve").await;
    harness.db_new_admin_staff(member_id).await;

    let user = harness
        .as_member_user(member_id, PermissionScope::empty())
        .await;

    let response = user.get("/admin/settings").await;
    response.assert_status(StatusCode::OK);

    let mut body = response.json::<EncodedSettings>();
    body.updated_at = Timestamp::from_secs(0).unwrap();

    assert_json_snapshot!(body);
    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_patch_route_without_sufficient_permissions(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).with_config_str(SETTINGS).build();
    harness.run_migrations().await;

    let member_id = Id::new(123456);
    harness.db_new_member(member_id, "steve").await;
    harness.db_new_admin_staff(member_id).await;

    let user = harness
        .as_member_user(member_id, PermissionScope::empty())
        .await;

    let response = user.patch("/admin/settings").json(&json!({})).await;
    response.assert_status(StatusCode::FORBIDDEN);

    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_patch_route_with_regular_users(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).with_config_str(SETTINGS).build();
    harness.run_migrations().await;

    let member_id = Id::new(123456);
    harness.db_new_member(member_id, "steve").await;

    let user = harness
        .as_member_user(member_id, PermissionScope::empty())
        .await;

    let response = user.patch("/admin/settings").json(&json!({})).await;
    response.assert_status(StatusCode::FORBIDDEN);

    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_get_route_for_unauthorized_users(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).with_config_str(SETTINGS).build();
    harness.run_migrations().await;

    let member_id = Id::new(123456);
    harness.db_new_member(member_id, "steve").await;

    let user = harness
        .as_member_user(member_id, PermissionScope::empty())
        .await;

    let response = user.get("/admin/settings").await;
    response.assert_status(StatusCode::FORBIDDEN);

    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_get_route_for_mc_server(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).with_config_str(SETTINGS).build();
    harness.run_migrations().await;

    let user = harness.as_minecraft_server().await;
    let response = user.get("/admin/settings").await;
    response.assert_status(StatusCode::UNAUTHORIZED);

    harness.assert_no_pending_jobs().await;
}
