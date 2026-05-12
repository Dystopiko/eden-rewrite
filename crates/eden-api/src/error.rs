use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use eden_api_types::error::{Error as ApiErrorType, ErrorCode};
use erased_report::ErasedReport;
use std::{borrow::Cow, sync::Arc};

#[derive(Debug)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: Cow<'static, str>,

    /// Additional headers to be embedded in the associated HTTP response.
    pub headers: Option<HeaderMap>,

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

        let status = match self.code {
            ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::ReadonlyMode | ErrorCode::ServiceUnavailable => {
                StatusCode::SERVICE_UNAVAILABLE
            }
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
            ErrorCode::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::Forbidden => StatusCode::FORBIDDEN,
        };

        let body: ApiErrorType = ApiErrorType {
            code: self.code,
            message: self.message.to_string(),
        };

        let mut response = (status, axum::Json(&body)).into_response();

        // Serialize without the report field
        if let Some(report) = report {
            response.extensions_mut().insert(report);
        }

        // Include every headers provided by the error
        if let Some(headers) = self.headers.take() {
            response.headers_mut().extend(headers);
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
