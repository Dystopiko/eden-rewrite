use axum::{
    extract::{Json, State},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use eden_api_types::me::CurrentUser;
use eden_model::tables::member_view::MemberView;
use eden_postgres::error::QueryResultExt;
use std::sync::Arc;

use crate::{
    ApiError, WebContext,
    auth::{ApiToken, ApiTokenType, AuthRequirement, check_for_authorization},
    convert::MemberFlagsExt,
};

pub async fn get(ctx: State<Arc<WebContext>>, parts: Parts) -> Result<Response, ApiError> {
    let token = check_for_authorization(&ctx, AuthRequirement::HasToken, &parts).await?;
    let current_user = match token.data {
        ApiTokenType::McServer => build_for_mc_server(&token),
        ApiTokenType::User { member_id, .. } => {
            let member = ctx
                .repository()
                .find_member_view(member_id)
                .await
                .optional()?
                .ok_or(ApiError::ACCESS_DENIED)?;

            build_for_member(&token, &member)
        }
    };

    Ok((StatusCode::OK, Json(current_user)).into_response())
}

fn build_for_mc_server(token: &ApiToken) -> CurrentUser {
    CurrentUser {
        id: None,
        name: format!("Minecraft Server ({})", token.token_id),
        rank: None,
        last_used_at: token.last_used_at,
    }
}

fn build_for_member(token: &ApiToken, member: &MemberView) -> CurrentUser {
    CurrentUser {
        id: Some(member.discord_user_id.cast()),
        name: member.name.to_string(),
        rank: Some(member.flags.api_name().to_string()),
        last_used_at: token.last_used_at,
    }
}
