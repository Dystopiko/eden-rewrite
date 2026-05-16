use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use eden_api_types::error::Error as ApiErrorType;
use eden_common::repository::QuerySettingsError;
use eden_postgres::{
    PoolError,
    error::{PgErrorType, PgReportExt},
};
use erased_report::ErasedReport;
use std::{borrow::Cow, fmt, sync::Arc};

#[derive(Debug)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: Cow<'static, str>,

    /// Additional headers to be embedded in the associated HTTP response.
    pub headers: Option<Box<HeaderMap>>,

    /// The original unhandled report, kept out of serialization and only
    /// used when converting this error into a [`Response`].
    pub report: Option<ErasedReport>,
}

impl ApiError {
    pub const INTERNAL: Self = Self::from_static(
        ErrorCode::Internal,
        "An unexpected error occurred while handling your request. \
        Please try again later, or contact a server administrator if the issue persists.",
    );

    pub const ACCESS_DENIED: Self = Self::from_static(ErrorCode::Unauthorized, "Access denied");

    pub const NOT_FOUND: Self = Self::from_static(
        ErrorCode::NotFound,
        "The requested resource could not be found.",
    );

    pub const READONLY_MODE: Self = Self::from_static(
        ErrorCode::ReadonlyMode,
        "Eden is temporarily operating in read-only mode. Check the announcements for \
        updates from the server administrator and try again later.",
    );

    pub const SERVICE_UNAVAILABLE: Self = Self::from_static(
        ErrorCode::ServiceUnavailable,
        "Eden is temporarily unavailable. Check the announcements for updates \
        from a server administrator and try again later.",
    );
}

impl ApiError {
    /// Creates a new [`ApiError`] with the given [`ErrorCode`] and static message.
    #[must_use]
    pub const fn from_static(code: ErrorCode, message: &'static str) -> Self {
        Self {
            code,
            message: Cow::Borrowed(message),
            headers: None,
            report: None,
        }
    }

    /// Creates a new [`ApiError`] with the given [`ErrorCode`] and owned message.
    #[must_use]
    pub fn from_owned(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: Cow::Owned(message.into()),
            headers: None,
            report: None,
        }
    }

    /// Attaches an unhandled report to this error.
    ///
    /// The report is stashed in the response extensions by [`IntoResponse`] so
    /// that `normalize_error` can log it with full span context. It is never
    /// serialized to the client.
    #[must_use]
    pub fn with_report(mut self, report: ErasedReport) -> Self {
        self.report = Some(report);
        self
    }
}

impl IntoResponse for ApiError {
    fn into_response(mut self) -> Response {
        let report = self.report.take().map(Arc::new);
        let status: StatusCode = self.code.into();

        let body: ApiErrorType = ApiErrorType {
            code: self.code.to_string(),
            message: self.message.to_string(),
        };

        let mut response = (status, axum::Json(&body)).into_response();

        // Serialize without the report field
        if let Some(report) = report {
            response.extensions_mut().insert(report);
        }

        // Include every headers provided by the error
        if let Some(headers) = self.headers.take() {
            response.headers_mut().extend(*headers);
        }

        response
    }
}

impl<R> From<R> for ApiError
where
    R: Into<ErasedReport>,
{
    fn from(value: R) -> Self {
        ApiError::INTERNAL.with_report(value.into())
    }
}

/// A machine-readable classification of an API error.
///
/// Serialized as `SCREAMING_SNAKE_CASE` in JSON responses (e.g. `"NOT_FOUND"`),
/// and mapped to an appropriate HTTP status code via [`From<ErrorCode> for StatusCode`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    /// Maps to `500 Internal Server Error`.
    Internal,
    /// Maps to `503 Service Unavailable`.
    ReadonlyMode,
    /// Maps to `409 Conflict`
    Conflict,
    /// Maps to `404 Not Found`.
    NotFound,
    /// Maps to `400 Bad Request`.
    InvalidRequest,
    /// Maps to `503 Service Unavailable`.
    ServiceUnavailable,
    /// Maps to `429 Too Many Requests`
    RateLimited,
    /// Maps to `401 Unauthorized`.
    Unauthorized,
    /// Maps to `403 Forbidden`
    Forbidden,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Internal => "INTERNAL",
            Self::ReadonlyMode => "READONLY_MODE",
            Self::Conflict => "CONFLICT",
            Self::NotFound => "NOT_FOUND",
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::ServiceUnavailable => "SERVICE_UNAVAILABLE",
            Self::RateLimited => "RATE_LIMITED",
            Self::Unauthorized => "UNAUTHORIZED",
            Self::Forbidden => "FORBIDDEN",
        })
    }
}

impl From<ErrorCode> for StatusCode {
    fn from(value: ErrorCode) -> Self {
        match value {
            ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Conflict => StatusCode::CONFLICT,
            ErrorCode::ReadonlyMode | ErrorCode::ServiceUnavailable => {
                StatusCode::SERVICE_UNAVAILABLE
            }
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
            ErrorCode::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::Forbidden => StatusCode::FORBIDDEN,
        }
    }
}

/// Classifies an [`ErasedReport`] into the most appropriate [`ApiError`].
///
/// Types that want to influence error classification should implement
/// [`HttpErrorClass`] and register themselves here. Unrecognized errors are
/// logged at `error` level and returned as [`ApiError::INTERNAL`].
pub fn classify(report: ErasedReport) -> ApiError {
    let classifiers: &[fn(&ErasedReport) -> Option<ApiError>] =
        &[classify_db, classify_query_settings];

    for classify in classifiers {
        if let Some(error) = classify(&report) {
            return error;
        }
    }

    tracing::error!(error = ?report, "unhandled error while processing request");
    ApiError::INTERNAL
}

fn classify_query_settings(report: &ErasedReport) -> Option<ApiError> {
    if let Some(error) = report.downcast_ref::<QuerySettingsError>() {
        tracing::warn!(?error);
    }
    None
}

fn classify_db(report: &ErasedReport) -> Option<ApiError> {
    let mut error = None;

    if let Some(kind) = report.pg_error_type() {
        error = Some(match kind {
            PgErrorType::RowNotFound => return Some(ApiError::NOT_FOUND),
            PgErrorType::Readonly => ApiError::READONLY_MODE,
            PgErrorType::UnhealthyConnection => ApiError::SERVICE_UNAVAILABLE,
            PgErrorType::Unknown => {
                tracing::error!(error = ?report, "encountered a database error");
                ApiError::INTERNAL
            }
            _ => return None,
        });
    }

    if let Some(pool_error) = report.downcast_ref::<PoolError>()
        && error.is_none()
    {
        error = Some(match pool_error {
            PoolError::General => {
                tracing::error!(error = ?report, "encountered a pool error");
                ApiError::INTERNAL
            }
            PoolError::Unhealthy => ApiError::SERVICE_UNAVAILABLE,
        });
    }

    error
}
