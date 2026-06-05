use axum::http::StatusCode;
use eden_api_types::{
    eden_minecraft_types::McEdition, eden_timestamp::Timestamp, types::FullMcAccount,
};
use eden_model::tables::{linked_mc_account::LinkMcAccount, tokens::PermissionScope};
use insta::{assert_json_snapshot, assert_snapshot};
use serde_json::json;
use twilight_model::id::Id;
use uuid::Uuid;

use crate::harness::TestHarness;

#[sqlx::test]
async fn should_link_mc_account(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).build();
    harness.run_migrations().await;

    let member_id = Id::new(123456);
    let uuid = Uuid::nil();
    harness.db_new_member(member_id, "steve").await;
    harness.db_new_admin_staff(member_id).await;

    let user = harness
        .as_member_user(member_id, PermissionScope::LINK_MINECRAFT_ACCOUNTS)
        .await;

    let response = user
        .post("/admin/members/123456/link")
        .json(&json!({
            "edition": McEdition::Java,
            "uuid": uuid,
            "username": "steve".to_string(),
        }))
        .await;

    response.assert_status(StatusCode::CREATED);

    let mut body = response.json::<FullMcAccount>();
    body.linked_at = Timestamp::from_secs(0).unwrap();

    assert_json_snapshot!(body);
    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_existing_mc_uuid_from_other_account(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).build();
    harness.run_migrations().await;

    let steve_member_id = Id::new(123456);
    let alex_member_id = Id::new(1);

    let uuid = Uuid::new_v4();
    harness.db_new_member(steve_member_id, "steve").await;
    harness.db_new_admin_staff(steve_member_id).await;

    harness.db_new_member(alex_member_id, "alex").await;

    let mut conn = harness.db_conn().await;
    LinkMcAccount::builder()
        .edition(McEdition::Java)
        .member_id(alex_member_id)
        .username("alex")
        .uuid(uuid)
        .build()
        .insert(&mut conn)
        .await
        .expect("failed to link minecraft account");

    drop(conn);

    let user = harness
        .as_member_user(steve_member_id, PermissionScope::LINK_MINECRAFT_ACCOUNTS)
        .await;

    let response = user
        .post("/admin/members/123456/link")
        .json(&json!({
            "edition": McEdition::Java,
            "uuid": uuid,
            "username": "steve".to_string(),
        }))
        .await;

    response.assert_status(StatusCode::CONFLICT);
    assert_snapshot!(response.text());

    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_existing_mc_uuid_from_the_same_account(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).build();
    harness.run_migrations().await;

    let member_id = Id::new(123456);
    let uuid = Uuid::new_v4();
    harness.db_new_member(member_id, "steve").await;
    harness.db_new_admin_staff(member_id).await;

    let mut conn = harness.db_conn().await;
    LinkMcAccount::builder()
        .edition(McEdition::Java)
        .member_id(member_id)
        .username("steve")
        .uuid(uuid)
        .build()
        .insert(&mut conn)
        .await
        .expect("failed to link minecraft account");

    drop(conn);

    let user = harness
        .as_member_user(member_id, PermissionScope::LINK_MINECRAFT_ACCOUNTS)
        .await;

    let response = user
        .post("/admin/members/123456/link")
        .json(&json!({
            "edition": McEdition::Java,
            "uuid": uuid,
            "username": "steve".to_string(),
        }))
        .await;

    response.assert_status(StatusCode::CONFLICT);
    assert_snapshot!(response.text());

    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_non_existing_member(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).build();
    harness.run_migrations().await;

    let member_id = Id::new(123456);
    harness.db_new_member(member_id, "steve").await;
    harness.db_new_admin_staff(member_id).await;

    let user = harness
        .as_member_user(member_id, PermissionScope::LINK_MINECRAFT_ACCOUNTS)
        .await;

    let response = user
        .post("/admin/members/1234/link")
        .json(&json!({
            "edition": McEdition::Java,
            "uuid": Uuid::new_v4().hyphenated(),
            "username": "steve".to_string(),
        }))
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_for_unauthorized_users(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).build();
    harness.run_migrations().await;

    let member_id = Id::new(123456);
    harness.db_new_member(member_id, "steve").await;

    let user = harness
        .as_member_user(member_id, PermissionScope::empty())
        .await;

    user.post("/admin/members/1234/link")
        .json(&json!({
            "edition": McEdition::Java,
            "uuid": Uuid::new_v4().hyphenated(),
            "username": "steve".to_string(),
        }))
        .await
        .assert_status(StatusCode::FORBIDDEN);

    harness.db_new_admin_staff(member_id).await;
    user.post("/admin/members/1234/link")
        .json(&json!({
            "edition": McEdition::Java,
            "uuid": Uuid::new_v4().hyphenated(),
            "username": "steve".to_string(),
        }))
        .await
        .assert_status(StatusCode::FORBIDDEN);

    harness.assert_no_pending_jobs().await;
}

#[sqlx::test]
async fn test_for_minecraft_server(pool: sqlx::PgPool) {
    let harness = TestHarness::builder(pool).build();
    harness.run_migrations().await;

    let user = harness.as_minecraft_server().await;
    let response = user
        .post("/admin/members/1234/link")
        .json(&json!({
            "edition": McEdition::Java,
            "uuid": Uuid::new_v4().hyphenated(),
            "username": "steve".to_string(),
        }))
        .await;

    response.assert_status(StatusCode::UNAUTHORIZED);
    harness.assert_no_pending_jobs().await;
}
