//! File-based diagnostic rendering for beautiful error messages.
//!
//! This crate provides a wrapper around [`codespan_reporting`] specifically designed
//! for rendering diagnostics from file sources with rich formatting and source code context.
//!
//! # Examples
//!
//! ```rust
//! use eden_file_diagnostics::{Renderer, codespan_reporting::diagnostic::{Diagnostic, Label}};
//!
//! let source = "let x = 42;";
//! let mut renderer = Renderer::new();
//! let file_id = renderer.add_file("example.rs", source);
//!
//! let diagnostic = Diagnostic::warning()
//!     .with_message("unused variable")
//!     .with_labels(vec![
//!         Label::primary(file_id, 4..5).with_message("variable `x` is never used")
//!     ]);
//!
//! match renderer.render(diagnostic) {
//!     Ok(rendered) => println!("{}", rendered),
//!     Err(e) => eprintln!("Failed to render diagnostic: {}", e),
//! }
//! ```
use std::{error::Error, fmt};

mod renderer;

pub use self::renderer::Renderer;
pub use codespan_reporting;

/// A rendered file diagnostic ready for display.
///
/// This type wraps a formatted diagnostic string that includes error messages,
/// file locations, source code context, and visual indicators. It implements
/// [`Display`] for easy printing and [`Error`] for use in error chains.
///
/// # Examples
///
/// ```rust
/// use eden_file_diagnostics::{Renderer, codespan_reporting::diagnostic::Diagnostic};
///
/// let renderer = Renderer::new().with_file("test.txt", "hello world");
/// let diagnostic = Diagnostic::note().with_message("Just a note");
///
/// if let Ok(rendered) = renderer.render(diagnostic) {
///     // Print to stdout
///     println!("{}", rendered);
///     
///     // Or convert to string
///     let message = rendered.to_string();
/// }
/// ```
#[derive(Debug, Clone)]
pub struct RenderedDiagnostic(String);

impl RenderedDiagnostic {
    /// Returns the rendered diagnostic as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the `RenderedDiagnostic` and returns the inner string.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for RenderedDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl Error for RenderedDiagnostic {}
