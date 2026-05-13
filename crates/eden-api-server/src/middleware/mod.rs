use axum::{Router, http::StatusCode, middleware::from_fn};
use std::{sync::Arc, time::Duration};
use tower_http::timeout::{RequestBodyTimeoutLayer, TimeoutLayer};

use crate::WebContext;

pub mod normalize_error;
pub mod trace_request;

pub fn apply(_ctx: Arc<WebContext>, router: Router<()>) -> Router<()> {
    let middleware = tower::ServiceBuilder::new()
        .layer(from_fn(trace_request::middleware))
        .layer(from_fn(normalize_error::middleware));

    router
        .layer(middleware)
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(15),
        ))
        .layer(RequestBodyTimeoutLayer::new(Duration::from_secs(30)))
}
