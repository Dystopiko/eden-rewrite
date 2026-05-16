use axum::{
    extract::{Json, State},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use eden_api_types::admin::settings::{EncodedSettings, PatchSettings};
use eden_model::tables::{
    settings::{NewSettings, Settings},
    tokens::PermissionScope,
};
use erased_report::IntoErasedReportExt;
use std::sync::Arc;

use crate::{
    ApiError, WebContext,
    auth::{AuthRequirement, check_for_authorization},
};

const GET_REQUIREMENT: AuthRequirement = AuthRequirement::User {
    admin: true,
    permissions: PermissionScope::empty(),
};

const PATCH_REQUIREMENT: AuthRequirement = AuthRequirement::User {
    admin: true,
    permissions: PermissionScope::EDIT_SETTINGS,
};

pub async fn get(ctx: State<Arc<WebContext>>, parts: Parts) -> Result<Response, ApiError> {
    check_for_authorization(&ctx, GET_REQUIREMENT, &parts).await?;

    let settings = ctx.repository().settings(&ctx.config()).await?;
    Ok((StatusCode::OK, Json(into_encoded_settings(settings))).into_response())
}

pub async fn patch(
    ctx: State<Arc<WebContext>>,
    parts: Parts,
    Json(body): Json<PatchSettings>,
) -> Result<Response, ApiError> {
    check_for_authorization(&ctx, PATCH_REQUIREMENT, &parts).await?;

    let settings = ctx.repository().settings(&ctx.config()).await?;
    let new_settings = try_update_settings(&ctx, &settings, &body).await?;
    ctx.cache().update_settings(&new_settings).await?;

    Ok((StatusCode::OK, Json(into_encoded_settings(new_settings))).into_response())
}

async fn try_update_settings(
    ctx: &WebContext,
    current: &Settings,
    patch: &PatchSettings,
) -> Result<Settings, ApiError> {
    let mut conn = ctx.pools().write().await?;
    let new_settings = NewSettings::builder()
        .allow_guests(patch.allow_guests.unwrap_or(current.allow_guests))
        .org_guild_id(current.org_guild_id.cast())
        .build()
        .upsert(&mut conn)
        .await?;

    conn.commit().await.erase_report()?;
    Ok(new_settings)
}

fn into_encoded_settings(settings: Settings) -> EncodedSettings {
    EncodedSettings {
        allow_guests: settings.allow_guests,
        updated_at: settings.updated_at.unwrap_or(settings.created_at),
    }
}
