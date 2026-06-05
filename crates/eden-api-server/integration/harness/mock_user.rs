use axum::http::header;
use axum_test::TestServer;
use eden_common::token::RawToken;
use std::ops::Deref;

use crate::harness::TestHarness;

/// A mock user for use in integration tests.
pub struct MockUser {
    server: TestServer,
}

impl MockUser {
    pub(super) fn new(harness: &TestHarness, token: Option<RawToken>) -> Self {
        let mut server = harness.server();
        if let Some(token) = token {
            server.add_header(header::AUTHORIZATION, format!("Bearer {}", token.expose()));
        }

        Self { server }
    }
}

impl Deref for MockUser {
    type Target = TestServer;

    fn deref(&self) -> &Self::Target {
        &self.server
    }
}
