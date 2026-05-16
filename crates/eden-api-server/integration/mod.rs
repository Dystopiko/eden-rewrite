mod system;

use crate::system::MockEdenSystem;
use axum_test::TestServer;
use eden_api_server::WebContext;
use eden_model::tables::tokens::PermissionScope;
use serde_json::json;
use std::sync::Arc;
use twilight_model::id::Id;

fn run_server(ctx: Arc<WebContext>) -> TestServer {
    let router = eden_api_server::router::build(ctx);
    TestServer::new(router)
}

#[sqlx::test]
async fn test_myself(pool: sqlx::PgPool) {
    let system = MockEdenSystem::builder(pool).build();
    system.run_migrations().await;
    system.db_new_member(Id::new(12345), "memothelemo").await;
    system.db_new_staff(Id::new(12345), true).await;

    let mut server = run_server(system.web_context().clone());
    let user = system
        .as_admin_user(Id::new(12345), PermissionScope::LINK_MINECRAFT_ACCOUNTS)
        .await;

    user.configure(&mut server);

    server
        .post("/admin/members/12345/link")
        .json(&json!({
            "edition": "java",
              "uuid": "00000000-0000-0000-0000-000000000000",
              "username": "steve"
        }))
        .await
        .assert_status_ok();
}
