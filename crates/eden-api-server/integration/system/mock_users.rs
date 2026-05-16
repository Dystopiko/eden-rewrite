use axum::http::{HeaderValue, header};
use axum_test::TestServer;
use eden_common::{AppContext, token::RawToken};
use std::sync::Arc;

#[must_use]
pub struct MockUser {
    app: Arc<AppContext>,
    token: Option<RawToken>,
}

impl MockUser {
    pub fn unauthenticated(app: &Arc<AppContext>) -> Self {
        Self {
            app: app.clone(),
            token: None,
        }
    }

    pub fn with_token(app: &Arc<AppContext>, token: RawToken) -> Self {
        Self {
            app: app.clone(),
            token: Some(token),
        }
    }

    pub fn configure(&self, server: &mut TestServer) {
        if let Some(token) = self.token.as_ref() {
            server.add_header(
                header::AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", token.expose())).unwrap(),
            );
        }
    }
}
