use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use eden_api_types::{admin::members::LinkMcAccount as LinkRequestBody, error::ErrorCode};
use eden_model::tables::{
    linked_mc_account::LinkMcAccount,
    mc_account_link_challenge::{ChallengeStatus, McAccountLinkChallenge},
};
use eden_postgres::error::QueryResultExt;
use eden_services::{
    discord::{ResolveMemberResult, setup_member},
    repository::CachedRepository,
};
use erased_report::ErasedReport;
use std::sync::Arc;
use twilight_model::id::{Id, marker::UserMarker};

use crate::{
    context::WebContext,
    controllers::{ApiResult, members::link::ensure_uuid_is_not_linked},
    error::ApiError,
};

pub async fn link(
    ctx: State<Arc<WebContext>>,
    Path(member_id): Path<Id<UserMarker>>,
    Json(body): Json<LinkRequestBody>,
) -> ApiResult<Response> {
    const ALREADY_LINKED_MSG: &str = "The specific UUID has already linked to an other player";

    let repository = CachedRepository::new(&*ctx.cache, &ctx.pools);
    ensure_uuid_is_not_linked(&repository, body.uuid.into(), ALREADY_LINKED_MSG).await?;

    let mut conn = ctx.pools.write().await?;
    let mut existing_challenge =
        McAccountLinkChallenge::find_in_progress(&mut conn, body.uuid.into())
            .await
            .optional()?;

    // Assume the challenge has been completed then we manually link player's
    // Minecraft account to their Discord account.
    if let Some(challenge) = existing_challenge.as_mut() {
        challenge
            .mark_status(&mut conn, ChallengeStatus::Done)
            .await?;

        let uuid = body.uuid.into_uuid();
        *challenge = McAccountLinkChallenge::find_in_progress(&mut conn, uuid).await?;
    }

    LinkMcAccount::builder()
        .member_id(member_id)
        .username(&body.username)
        .uuid(body.uuid.into())
        .edition(body.edition)
        .build()
        .insert(&mut conn)
        .await?;

    conn.commit().await.map_err(ErasedReport::new)?;

    repository
        .cache
        .invalidate_linked_account_view(body.uuid.into())
        .await?;

    if let Some(challenge) = existing_challenge {
        repository.cache.update_link_challenge(&challenge).await?;
    }

    Ok(StatusCode::NO_CONTENT.into_response())
}

pub async fn post(
    ctx: State<Arc<WebContext>>,
    Path(member_id): Path<Id<UserMarker>>,
) -> ApiResult<Response> {
    let config = ctx.config.get();
    let Some((discord, config)) = ctx
        .discord
        .as_ref()
        .zip(config.organization.discord.as_ref())
    else {
        return Err(ApiError::from_static(
            ErrorCode::Internal,
            "Discord service is disabled",
        ));
    };

    let result = discord.resolve_member_from_org_guild(member_id).await?;
    let member = match result {
        ResolveMemberResult::BotNotAddedInGuild => {
            return Err(ApiError::from_static(
                ErrorCode::Internal,
                "Eden discord bot may not be in the configured organization guild",
            ));
        }
        ResolveMemberResult::MemberNotAddedInGuild => {
            return Err(ApiError::from_static(
                ErrorCode::Internal,
                "A specified member may not be in the configured organization guild",
            ));
        }
        ResolveMemberResult::Done(member) => member,
    };

    let mut conn = ctx.pools.write().await?;
    setup_member(&mut conn, config, &member).await?;

    // let repository = CachedRepository::new(&*ctx.cache, &ctx.pools);
    // let member = repository.find_member_view(member_id).await?;

    // into_full_member(member, Vec::new());

    todo!()
}
