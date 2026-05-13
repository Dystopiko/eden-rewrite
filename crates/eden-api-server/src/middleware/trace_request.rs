use axum::{
    extract::{Extension, FromRequestParts, MatchedPath, Request},
    http::{HeaderName, HeaderValue, Method, StatusCode, Uri},
    middleware::Next,
    response::IntoResponse,
};
use axum_extra::{TypedHeader, headers::UserAgent};
use std::time::Instant;
use tracing::Instrument;
use uuid::Uuid;

/// A request-scoped unique identifier injected by [`middleware`] into both
/// request and response extensions.
///
/// Included in error responses as `X-Request-ID` so that clients can correlate
/// a reported problem with server-side logs.
#[derive(Clone, Copy)]
pub struct RequestId(pub Uuid);

#[derive(FromRequestParts)]
pub struct RequestMetadata {
    method: Method,
    uri: Uri,
    matched_path: Option<Extension<MatchedPath>>,
    user_agent: Option<TypedHeader<UserAgent>>,
}

const X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

/// Records a structured tracing span for each incoming HTTP request and appends
/// an `X-Request-ID` header to non-routing-failure responses.
pub async fn middleware(
    metadata: RequestMetadata,
    mut req: Request,
    next: Next,
) -> impl IntoResponse {
    let id = Uuid::new_v4();
    req.extensions_mut().insert(RequestId(id));

    let matched_path = metadata
        .matched_path
        .as_ref()
        .map(|p| p.0.as_str())
        .unwrap_or_default();

    let user_agent = metadata.user_agent.as_ref().map(|v| v.as_str());

    let span = tracing::info_span!(
        "http.request",
        request.id = %id,
        request.method = %metadata.method,
        request.uri = %metadata.uri,
        request.path = %matched_path,
        request.user_agent = ?user_agent,
    );

    let start = Instant::now();
    let mut response = next.run(req).instrument(span.clone()).await;
    let duration = start.elapsed();

    // Omit request IDs from generic routing failures — these are not
    // correlated with any server-side work worth tracing.
    let status = response.status();
    if status != StatusCode::NOT_FOUND && status != StatusCode::METHOD_NOT_ALLOWED {
        let header_value = HeaderValue::from_str(&id.to_string())
            .expect("UUID should always produce a valid UTF-8 string");

        response.extensions_mut().insert(RequestId(id));
        response.headers_mut().insert(X_REQUEST_ID, header_value);
    }

    span.in_scope(|| {
        tracing::trace!(
            "{method} {uri} -> {status} ({duration:?})",
            method = metadata.method,
            uri = metadata.uri,
            status = status.as_str(),
        );
    });

    response
}
