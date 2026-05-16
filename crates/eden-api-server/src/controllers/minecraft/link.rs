use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use eden_api_types::{
    eden_minecraft_types::McEdition,
    members::link::{LinkMcAccount, LinkMcAccountChallenge},
};
use eden_common::challenge_code::{HashedChallengeCode, RawChallengeCode};
use eden_model::tables::{
    linked_mc_account::LinkedMcAccount,
    mc_account_link_challenge::{McAccountLinkChallenge, NewMcLinkChallenge},
};
use eden_postgres::error::QueryResultExt;
use erased_report::IntoErasedReportExt;
use error_stack::ResultExt;
use std::{sync::Arc, time::Duration};
use thiserror::Error;
use tokio::task::spawn_blocking;

use crate::{ApiError, WebContext, error::ErrorCode};

const CHALLENGE_TTL: Duration = Duration::from_mins(5);

pub async fn post(
    ctx: State<Arc<WebContext>>,
    Json(body): Json<LinkMcAccount>,
) -> Result<Response, ApiError> {
    let mut conn = ctx.pools().write().await?;

    ensure_mc_account_not_already_linked(&mut conn, &body).await?;
    ensure_no_pending_link_challenge(&mut conn, &body).await?;

    let code = generate_challenge_code(body.edition).await?;
    let hashed = code.hash();

    let challenge = insert_link_challenge(&mut conn, &hashed, &body).await?;
    let response = LinkMcAccountChallenge {
        code: code.expose().to_string(),
        expires_at: challenge.expires_at,
    };

    conn.commit().await.erase_report()?;
    Ok((StatusCode::CREATED, Json(response)).into_response())
}

async fn insert_link_challenge(
    conn: &mut eden_postgres::Transaction<'_>,
    hashed_code: &HashedChallengeCode,
    body: &LinkMcAccount,
) -> Result<McAccountLinkChallenge, ApiError> {
    let challenge = NewMcLinkChallenge::builder()
        .hashed_code(&hashed_code.encode())
        .ttl(CHALLENGE_TTL)
        .player_uuid(body.uuid.into_uuid())
        .username(&body.username)
        .edition(body.edition)
        .ip_address(body.ip)
        .build()
        .insert(conn)
        .await?;

    Ok(challenge)
}

async fn generate_challenge_code(edition: McEdition) -> Result<RawChallengeCode, ApiError> {
    #[derive(Debug, Error)]
    #[error("challenge code generator thread panicked")]
    struct GeneratorPanicked;

    spawn_blocking(move || match edition {
        McEdition::Java => RawChallengeCode::generate_for_java(),
        McEdition::Bedrock => RawChallengeCode::generate_for_bedrock(),
    })
    .await
    .change_context(GeneratorPanicked)?
    .map_err(ApiError::from)
}

async fn ensure_no_pending_link_challenge(
    conn: &mut eden_postgres::Transaction<'_>,
    body: &LinkMcAccount,
) -> Result<(), ApiError> {
    let uuid = body.uuid.into_uuid();
    let has_existing_challenge = McAccountLinkChallenge::find_in_progress(conn, uuid)
        .await
        .optional()?
        .is_some();

    if has_existing_challenge {
        return Err(ApiError::from_static(
            ErrorCode::Conflict,
            "You have already a pending link challenge. Submit the code to the Eden Discord bot
            in direct message or wait for it to expire.",
        ));
    }

    Ok(())
}

async fn ensure_mc_account_not_already_linked(
    conn: &mut eden_postgres::Transaction<'_>,
    body: &LinkMcAccount,
) -> Result<(), ApiError> {
    let already_linked = LinkedMcAccount::from_mc_uuid(body.uuid.into_uuid(), conn)
        .await
        .optional()?
        .is_some();

    if already_linked {
        return Err(ApiError::from_static(
            ErrorCode::Conflict,
            "This Minecraft account is already linked to a Discord account.",
        ));
    }

    Ok(())
}
