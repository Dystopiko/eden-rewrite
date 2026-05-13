use axum::{
    Router,
    http::{Method, StatusCode},
    response::IntoResponse,
    routing::post,
};
use std::sync::Arc;

use crate::{ApiError, WebContext, controllers::*};

pub fn build(ctx: Arc<WebContext>) -> Router<()> {
    let router = Router::new().route("/minecraft/login", post(minecraft::login::post));

    let router = router
        .fallback(async |method: Method| match method {
            Method::HEAD => StatusCode::NOT_FOUND.into_response(),
            _ => ApiError::NOT_FOUND.into_response(),
        })
        .with_state(ctx.clone());

    crate::middleware::apply(ctx, router)
}
