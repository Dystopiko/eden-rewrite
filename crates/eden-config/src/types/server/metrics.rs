use doku::Document;
use eden_file_diagnostics::RenderedDiagnostic;
use serde::Deserialize;

use crate::{context::SourceContext, validation::Validate};

#[derive(Clone, Debug, Deserialize, Document, Eq, PartialEq)]
#[serde(default)]
pub struct Metrics {
    /// Enable or disable Prometheus metrics collection.
    pub enabled: bool,
}

impl Validate for Metrics {
    fn validate(&self, _ctx: &SourceContext<'_>) -> Result<(), RenderedDiagnostic> {
        Ok(())
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self { enabled: true }
    }
}
