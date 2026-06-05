use axum::{
    extract::{Json, State},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use eden_api_types::{
    eden_timestamp::Timestamp,
    sessions::{RequestSession, SessionGranted},
    types::{EncodedMember, MinimalMemberStatus},
};
use eden_common::{CachedRepository, domain::notifier::LinkedMcAccountLogin};
use eden_jobs::{
    internal_alerts::{AlertGuestJoinedJob, AlertRevokedLoginJob},
    notification::NotifyPendingLoginJob,
};
use eden_model::{
    common::ApprovalStatus,
    tables::{
        linked_mc_account_view::LinkedMcAccountView, mc_login_event::NewMcLoginEvent,
        member_view::MemberView,
    },
};
use eden_postgres::error::QueryResultExt;
use erased_report::{ErasedReport, IntoErasedReportExt};
use std::sync::Arc;

use crate::{
    ApiError, WebContext,
    auth::{AuthRequirement, check_for_authorization},
    convert::{LinkedMcAccountViewExt, MemberFlagsExt},
    error::ErrorCode,
};

pub async fn post(
    ctx: State<Arc<WebContext>>,
    parts: Parts,
    Json(body): Json<RequestSession>,
) -> Result<Response, ApiError> {
    check_for_authorization(&ctx, AuthRequirement::McServer, &parts).await?;

    let repository = ctx.repository();
    let account = repository
        .find_linked_mc_account_view(body.uuid.into_uuid())
        .await
        .optional()?;

    let (response, query) = if let Some(account) = account {
        grant_member_access(&ctx, &repository, &account, &body).await?
    } else {
        grant_guest_access(&ctx, &repository, &body).await?
    };

    log_successful_login(&ctx, query).await;
    Ok((StatusCode::OK, Json(response)).into_response())
}

async fn log_successful_login(ctx: &WebContext, query: NewMcLoginEvent) {
    let result = async {
        let mut conn = ctx.pools().write().await?;
        let event = query.insert(&mut conn).await?;
        conn.commit().await.erase_report()?;

        Ok::<_, ErasedReport>(event)
    };

    match result.await {
        Ok(event) => {
            ctx.job_queue()
                .enqueue_job(AlertGuestJoinedJob(event))
                .await;
        }
        Err(error) => tracing::warn!(?error, "failed to log successful login event"),
    }
}

async fn grant_member_access(
    ctx: &WebContext,
    repository: &CachedRepository<'_>,
    account: &LinkedMcAccountView,
    body: &RequestSession,
) -> Result<(SessionGranted, NewMcLoginEvent), ApiError> {
    let member = &account.member;

    check_if_member_trusts_this_ip(ctx, repository, account, body).await?;
    validate_account_edition_used(account, body)?;

    let perks = ctx.minecraft().resolve_perks(
        member.flags,
        Some(member.discord_user_id.cast()),
        Some(account.uuid),
    );

    let query = NewMcLoginEvent::from_linked(&account.simplify())
        .ip_address(body.ip)
        .build();

    let response = build_member_session(member, account, perks);
    Ok((response, query))
}

async fn grant_guest_access(
    ctx: &WebContext,
    repository: &CachedRepository<'_>,
    body: &RequestSession,
) -> Result<(SessionGranted, NewMcLoginEvent), ApiError> {
    let settings = repository.settings(&ctx.config()).await?;
    if !settings.allow_guests {
        return Err(ApiError::from_static(
            ErrorCode::GuestAccessDisabled,
            "Guest access is disabled by an administrator",
        ));
    }

    let query = NewMcLoginEvent::builder()
        .player_uuid(body.uuid.into_uuid())
        .ip_address(body.ip)
        .edition(body.edition)
        .build();

    let response = SessionGranted {
        member: None,
        perks: Vec::new(),
    };

    Ok((response, query))
}

fn build_member_session(
    member: &MemberView,
    account: &LinkedMcAccountView,
    perks: Vec<String>,
) -> SessionGranted {
    SessionGranted {
        member: Some(EncodedMember {
            id: member.discord_user_id.cast(),
            name: member.name.to_string(),
            status: Some(MinimalMemberStatus::Okay),
            last_login_at: account.last_login_at,
            rank: Some(member.flags.api_name().to_string()),
        }),
        perks,
    }
}

async fn check_if_member_trusts_this_ip(
    ctx: &WebContext,
    repository: &CachedRepository<'_>,
    account: &LinkedMcAccountView,
    body: &RequestSession,
) -> Result<(), ApiError> {
    let member_id = account.member.discord_user_id.cast();
    let result = repository
        .resolve_member_cidr_trust(member_id, body.ip)
        .await?;

    let metadata = LinkedMcAccountLogin {
        created_at: Timestamp::now(),
        member_id,
        ip: body.ip,
        edition: body.edition,
        username: account.username.clone(),
        uuid: account.uuid,
    };

    match result.value.status {
        ApprovalStatus::Approved => Ok(()),
        ApprovalStatus::Pending => {
            if result.created {
                ctx.job_queue()
                    .enqueue_job(NotifyPendingLoginJob(metadata))
                    .await;
            }

            Err(ApiError::from_static(
                ErrorCode::Forbidden,
                "Unrecognized IP address detected. Check for Eden notifications to approve \
                or block this login attempt.",
            ))
        }
        ApprovalStatus::Revoked => {
            ctx.job_queue()
                .enqueue_job(AlertRevokedLoginJob(metadata))
                .await;

            Err(ApiError::from_static(
                ErrorCode::Forbidden,
                "Your IP address has been blocked from accessing this account. \
                Please contact support if you believe this is a mistake.",
            ))
        }
    }
}

fn validate_account_edition_used(
    account: &LinkedMcAccountView,
    body: &RequestSession,
) -> Result<(), ApiError> {
    if account.edition != body.edition {
        return Err(ApiError::from_static(
            ErrorCode::InvalidRequest,
            "Incompatible edition",
        ));
    }

    Ok(())
}
