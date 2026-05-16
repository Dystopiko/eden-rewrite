use axum::{
    extract::{Json, Path, State},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use eden_api_types::{admin::members::LinkMcAccount as RequestBody, types::FullMcAccount};
use eden_model::tables::{
    linked_mc_account::{LinkMcAccount, LinkedMcAccount},
    tokens::PermissionScope,
};
use eden_postgres::error::{PgErrorType, PgResultExt};
use std::sync::Arc;
use twilight_model::id::{Id, marker::UserMarker};

use crate::{
    ApiError, WebContext,
    auth::{AuthRequirement, check_for_authorization},
    error::ErrorCode,
};

const REQUIREMENT: AuthRequirement = AuthRequirement::User {
    admin: true,
    permissions: PermissionScope::LINK_MINECRAFT_ACCOUNTS,
};

pub async fn post(
    ctx: State<Arc<WebContext>>,
    parts: Parts,
    Path(member_id): Path<Id<UserMarker>>,
    Json(body): Json<RequestBody>,
) -> Result<Response, ApiError> {
    check_for_authorization(&ctx, REQUIREMENT, &parts).await?;

    let mut conn = ctx.pools().write().await?;
    ctx.repository().find_member_view(member_id).await?;

    let account = try_insert_linked_mc_account(&mut conn, member_id, &body).await?;
    let response = build_full_mc_account(&body, &account);

    Ok((StatusCode::CREATED, Json(response)).into_response())
}

async fn try_insert_linked_mc_account(
    conn: &mut eden_postgres::Transaction<'_>,
    member_id: Id<UserMarker>,
    body: &RequestBody,
) -> Result<LinkedMcAccount, ApiError> {
    let result = LinkMcAccount::builder()
        .username(&body.username)
        .edition(body.edition)
        .member_id(member_id)
        .uuid(body.uuid.into_uuid())
        .build()
        .insert(conn)
        .await;

    match result.pg_error_type() {
        Some(PgErrorType::UniqueViolation(msg)) if msg.contains("uuid") => {
            Err(ApiError::from_static(
                ErrorCode::Conflict,
                "This specified UUID is already linked to another member.",
            ))
        }
        Some(PgErrorType::UniqueViolation(msg)) if msg.contains("pk") => {
            Err(ApiError::from_static(
                ErrorCode::Conflict,
                "This specified Minecraft account is already linked to this member.",
            ))
        }
        _ => Ok(result?),
    }
}

fn build_full_mc_account(body: &RequestBody, account: &LinkedMcAccount) -> FullMcAccount {
    FullMcAccount {
        uuid: body.uuid,
        username: body.username.clone(),
        edition: body.edition,
        linked_at: account.linked_at,
        last_login_at: None,
        last_ip_address: None,
    }
}
