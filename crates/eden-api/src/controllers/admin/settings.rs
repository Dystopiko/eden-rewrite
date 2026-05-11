use axum::{
    extract::{Json, State},
    response::{IntoResponse, Response},
};
use eden_api_types::admin::settings::PatchSettings;
use eden_config::types::setup::InitialSettings;
use eden_services::repository::CachedRepository;
use std::sync::Arc;

use crate::{context::WebContext, controllers::ApiResult, convert::into_encoded_settings};

pub async fn get(ctx: State<Arc<WebContext>>) -> ApiResult<Response> {
    let repository = CachedRepository::new(&*ctx.cache, &ctx.pools);
    let current = repository.settings(&ctx.config.get()).await?;
    Ok(Json(into_encoded_settings(current)).into_response())
}

pub async fn patch(
    ctx: State<Arc<WebContext>>,
    Json(body): Json<PatchSettings>,
) -> ApiResult<Response> {
    let repository = CachedRepository::new(&*ctx.cache, &ctx.pools);
    let current = repository.settings(&ctx.config.get()).await?;

    let patch: InitialSettings = InitialSettings {
        allow_guests: body.allow_guests.unwrap_or(current.allow_guests),
    };

    let new = repository
        .update_settings(&ctx.config.get(), &patch)
        .await?;

    Ok(Json(into_encoded_settings(new)).into_response())
}
