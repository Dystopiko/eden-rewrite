use axum::extract::Json;
use serde_json::json;

use crate::error::ApiError;

pub mod alerts;
pub mod metrics;
pub mod sessions;

pub async fn index() -> Json<serde_json::Value> {
    Json(json!({ "hello": "world" }))
}

/// The standard `Result` type for API handlers.
pub type ApiResult<T> = std::result::Result<T, ApiError>;
