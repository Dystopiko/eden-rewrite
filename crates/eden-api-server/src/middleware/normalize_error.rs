use erased_report::ErasedReport;
use error_stack::{Report, ResultExt};
use mime::Mime;
use std::borrow::Cow;
use std::str::FromStr;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

use axum::{
    extract::{Json, Request},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{ApiError, error::ErrorCode};

#[derive(Debug, Error)]
#[error("failed to convert plain-text error body to JSON")]
struct JsonifyError;

/// Normalizes all error responses into a consistent [`ApiError`] JSON payload.
pub async fn middleware(request: Request, next: Next) -> impl IntoResponse {
    let mut response = next.run(request).await;

    let status = response.status();
    if !status.is_client_error() && !status.is_server_error() {
        return response;
    }

    if let Some(report) = take_erased_report(&mut response) {
        return crate::error::classify(report).into_response();
    }

    match jsonify_plain_text_error(response).await {
        Ok(converted) => converted,
        Err(report) => {
            tracing::error!(error = ?report, "failed to jsonify plain-text error response");
            ApiError::INTERNAL.into_response()
        }
    }
}

fn take_erased_report(res: &mut Response) -> Option<ErasedReport> {
    res.extensions_mut()
        .remove::<Arc<ErasedReport>>()
        .and_then(Arc::into_inner)
}

async fn jsonify_plain_text_error(res: Response) -> Result<Response, Report<JsonifyError>> {
    const MAX_ERROR_BODY_BYTES: usize = 1_000_000;

    let (mut parts, body) = res.into_parts();
    if !is_plain_text_utf8(&parts.headers) {
        return Ok((parts, body).into_response());
    }

    let bytes = axum::body::to_bytes(body, MAX_ERROR_BODY_BYTES)
        .await
        .change_context(JsonifyError)?;

    let message = String::from_utf8(bytes.into()).change_context(JsonifyError)?;
    parts.headers.remove(header::CONTENT_TYPE);
    parts.headers.remove(header::CONTENT_LENGTH);

    let error = ApiError::from_owned(ErrorCode::InvalidRequest, message);
    Ok((parts, error).into_response())
}

fn is_plain_text_utf8(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| Mime::from_str(v).ok())
        .is_some_and(|mime| mime == mime::TEXT_PLAIN_UTF_8)
}
