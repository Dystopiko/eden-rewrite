//! TOML parsing utilities with beautiful error diagnostics.
//!
//! This crate provides helper functions for parsing TOML files with rich error messages
//! with the help of [`eden_file_diagnostics`].
use eden_file_diagnostics::{
    RenderedDiagnostic, Renderer,
    codespan_reporting::diagnostic::{Diagnostic, Label},
};
use serde::de::DeserializeOwned;
use std::path::Path;
use toml_edit::Document;

/// Parses a TOML string into a [`Document`].
pub fn parse_as_document(
    contents: &str,
    path: &Path,
) -> Result<Document<String>, RenderedDiagnostic> {
    Document::parse(contents.to_owned())
        .map_err(|error| create_diagnostic(error.into(), contents, path))
}

/// Deserializes a TOML [`Document`] into a Rust type.
pub fn deserialize<T: DeserializeOwned>(
    document: &Document<String>,
    path: &Path,
) -> Result<T, RenderedDiagnostic> {
    toml_edit::de::from_document(document.clone())
        .map_err(|error| create_diagnostic(error, document.raw(), path))
}

fn create_diagnostic(error: toml_edit::de::Error, source: &str, path: &Path) -> RenderedDiagnostic {
    let mut diagnostic = Diagnostic::error().with_message(error.message());
    if let Some(span) = error.span() {
        let label = Label::primary(0, span).with_message(error.message());
        diagnostic = diagnostic.with_label(label);
    }

    Renderer::new()
        .with_file(&path.to_string_lossy(), source)
        .render(diagnostic)
        .expect("rendering should succeed with valid file data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_emit() {
        let contents: &str = "hello";
        let path = Path::new("hello.rs");

        let diagnostic = parse_as_document(contents, path).unwrap_err();
        insta::assert_snapshot!(diagnostic);
    }
}
