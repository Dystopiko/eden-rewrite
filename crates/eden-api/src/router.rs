use axum::{
    http::{Method, StatusCode},
    response::IntoResponse,
    routing::{Router, get, post},
};
use std::sync::Arc;

use crate::{context::WebContext, controllers::*, error::ApiError};

pub fn build(ctx: Arc<WebContext>) -> Router<()> {
    let router = Router::new()
        .route("/", get(index))
        .route("/admin/members/{id}", post(admin::members::id::post))
        .route("/admin/members/{id}/link", post(admin::members::id::link))
        .route(
            "/admin/settings",
            get(admin::settings::get).patch(admin::settings::patch),
        )
        .route("/alerts/commands", post(alerts::commands::post))
        .route("/metrics", get(metrics::get))
        .route("/sessions", post(sessions::post::post));

    let router = router
        .fallback(async |method: Method| match method {
            Method::HEAD => StatusCode::NOT_FOUND.into_response(),
            _ => ApiError::NOT_FOUND.into_response(),
        })
        .with_state(ctx.clone());

    crate::middleware::apply(ctx, router)
}
