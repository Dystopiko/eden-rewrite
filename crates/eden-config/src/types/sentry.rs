//! Sentry error tracking and performance monitoring configuration.
//!
//! This module defines the configuration for integrating with Sentry.io,
//! providing error tracking, performance monitoring, and crash reporting
//! for the Eden backend.
use doku::Document;
use eden_config_derive::Validate;
use eden_file_diagnostics::RenderedDiagnostic;
use eden_sensitive::Sensitive;
use sentry_core::types::Dsn;
use serde::Deserialize;

use crate::validation::{ValidationContext, create_field_error};

/// Sentry integration configuration.
///
/// This structure configures the Sentry SDK for error tracking and
/// performance monitoring. All fields are required when the `sentry`
/// section is present in the configuration.
///
/// To disable Sentry entirely, omit this section from the configuration.
#[derive(Clone, Debug, Deserialize, Document, PartialEq, Validate)]
pub struct Sentry {
    #[doku(
        as = "String",
        example = "https://examplePublicKey@o0.ingest.sentry.io/0"
    )]
    #[validate(skip)]
    pub dsn: Sensitive<Dsn>,

    /// The environment name for this deployment.
    ///
    /// This helps distinguish between different deployment environments
    /// (e.g., `"production"`, `"staging"`, `"development"`) in the Sentry
    /// dashboard, enabling environment-specific filtering and alerting.
    ///
    /// Must not be empty.
    #[doku(example = "production")]
    #[validate(with = "validate_environment")]
    pub environment: String,

    /// Sample rate for performance monitoring traces.
    ///
    /// Defaults to `1.0` if not specified.
    #[doku(example = "1.0")]
    #[serde(default = "default_traces_sample_rate")]
    #[validate(with = "validate_traces_sample_rate")]
    pub traces_sample_rate: f32,

    /// Log level filter for Sentry events.
    ///
    /// Specifies which log levels should be captured and sent to Sentry.
    /// Common values: "error", "warn", "info", "debug", "trace".
    ///
    /// Defaults to `"info"` if not specified.
    #[doku(example = "info")]
    #[serde(default = "default_targets")]
    #[validate(skip)]
    pub targets: String,
}

fn default_targets() -> String {
    String::from("info")
}

const fn default_traces_sample_rate() -> f32 {
    1.0
}

fn validate_environment(
    value: &str,
    ctx: &ValidationContext<'_>,
) -> Result<(), RenderedDiagnostic> {
    static NOTE: &str = "Common values: \"production\", \"staging\", \"development\"";
    if value.is_empty() {
        return Err(create_field_error(
            ctx,
            &["sentry", "environment"],
            "Sentry environment must not be empty",
            |diagnostic| {
                diagnostic.notes.push(NOTE.into());
            },
        ));
    }
    Ok(())
}

fn validate_traces_sample_rate(
    &value: &f32,
    ctx: &ValidationContext<'_>,
) -> Result<(), RenderedDiagnostic> {
    if (0.0..=1.0).contains(&value) {
        return Ok(());
    }

    Err(create_field_error(
        ctx,
        &["sentry", "traces_sample_rate"],
        "traces_sample_rate must be within range of 0.0 to 1.0",
        |diagnostic| {
            let note = format!(
                "Got: {}, expected a value between 0.0 (no traces) and 1.0 (all traces)",
                value
            );
            diagnostic.notes.push(note);
        },
    ))
}
