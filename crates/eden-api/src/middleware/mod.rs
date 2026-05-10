use axum::{Router, http::StatusCode};
use std::{sync::Arc, time::Duration};
use tower_http::timeout::{RequestBodyTimeoutLayer, TimeoutLayer};

use crate::context::WebContext;

pub fn apply(_ctx: Arc<WebContext>, router: Router<()>) -> Router<()> {
    router
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(RequestBodyTimeoutLayer::new(Duration::from_secs(30)))
}
