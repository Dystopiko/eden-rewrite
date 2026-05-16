use axum::http::{header, request::Parts};
use eden_api_types::eden_timestamp::Timestamp;
use eden_common::token::{HashedToken, RawToken};
use eden_model::tables::tokens::{PermissionScope, Token, TokenType};
use eden_postgres::error::QueryResultExt;
use error_stack::Report;
use thiserror::Error;
use twilight_model::id::{Id, marker::UserMarker};
use uuid::Uuid;

use crate::{ApiError, WebContext, error::ErrorCode};

#[derive(Clone, Debug)]
pub struct ApiToken {
    pub token_id: Uuid,
    pub created_at: Timestamp,
    pub last_used_at: Option<Timestamp>,
    pub authorized_by: String,
    pub expires_at: Option<Timestamp>,
    pub revoked: bool,
    pub data: ApiTokenType,
}

#[derive(Debug, Error)]
#[error("unexpected missing data from a user token")]
pub struct MissingUserTokenData;

impl ApiToken {
    pub fn from_db(token: &Token) -> Result<Self, Report<MissingUserTokenData>> {
        let data = match token.kind {
            TokenType::McServer => ApiTokenType::McServer,
            TokenType::User => {
                let (member_id, permissions) = token
                    .member_id
                    .zip(token.permissions)
                    .ok_or_else(|| Report::new(MissingUserTokenData))?;

                ApiTokenType::User {
                    member_id: member_id.cast(),
                    permissions,
                }
            }
        };

        Ok(Self {
            token_id: token.id,
            created_at: token.created_at,
            last_used_at: token.last_used_at,
            authorized_by: token.authorized_by.clone(),
            expires_at: token.expires_at,
            revoked: token.revoked,
            data,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ApiTokenType {
    McServer,
    User {
        member_id: Id<UserMarker>,
        permissions: PermissionScope,
    },
}

#[derive(Clone, Debug)]
pub enum AuthRequirement {
    HasToken,
    McServer,
    User {
        admin: bool,
        permissions: PermissionScope,
    },
}

pub async fn check_for_authorization(
    ctx: &WebContext,
    requirement: AuthRequirement,
    parts: &Parts,
) -> Result<ApiToken, ApiError> {
    let raw = extract_token_from_bearer(parts).ok_or(ApiError::ACCESS_DENIED)?;

    let mut db_token = fetch_valid_token(ctx, raw.hash()).await?;
    let api_token = ApiToken::from_db(&db_token)?;

    should_meet_requirement(ctx, requirement, &api_token).await?;
    track_token_usage(ctx, &mut db_token).await?;

    Ok(api_token)
}

fn extract_token_from_bearer(parts: &Parts) -> Option<RawToken> {
    let auth_header = parts.headers.get(header::AUTHORIZATION)?;
    let auth_header = auth_header.to_str().ok()?;
    let (scheme, token) = auth_header.split_once(' ').unwrap_or(("", auth_header));
    if !(scheme.eq_ignore_ascii_case("Bearer") || scheme.is_empty()) {
        return None;
    }
    RawToken::parse(token.trim_ascii().to_string())
}

async fn fetch_valid_token(ctx: &WebContext, hash: HashedToken) -> Result<Token, ApiError> {
    let token = ctx
        .repository()
        .find_token(&hash)
        .await
        .optional()?
        .ok_or(ApiError::ACCESS_DENIED)?;

    if token.revoked {
        return Err(ApiError::ACCESS_DENIED);
    }

    Ok(token)
}

async fn should_meet_requirement(
    ctx: &WebContext,
    requirement: AuthRequirement,
    api_token: &ApiToken,
) -> Result<(), ApiError> {
    match (requirement, &api_token.data) {
        (AuthRequirement::HasToken, ..) => Ok(()),
        (AuthRequirement::McServer, ApiTokenType::McServer) => Ok(()),
        (
            AuthRequirement::User {
                admin: requires_admin,
                permissions: requirements,
            },
            ApiTokenType::User {
                member_id,
                permissions,
            },
        ) => {
            let member = ctx
                .repository()
                .find_member_view(*member_id)
                .await
                .optional()?
                .ok_or(ApiError::ACCESS_DENIED)?;

            if requires_admin && !member.flags.is_admin() {
                return Err(ApiError::from_static(ErrorCode::Forbidden, "Access denied"));
            }

            if !permissions.has(requirements) {
                return Err(ApiError::from_static(ErrorCode::Forbidden, "Access denied"));
            }

            Ok(())
        }
        _ => Err(ApiError::ACCESS_DENIED),
    }
}

async fn track_token_usage(ctx: &WebContext, token: &mut Token) -> Result<(), ApiError> {
    let mut conn = ctx.pools().read_prefer_primary().await?;
    _ = token.update_last_used_at(&mut conn).await;

    Ok(())
}
