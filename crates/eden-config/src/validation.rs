use eden_file_diagnostics::{
    RenderedDiagnostic, Renderer,
    codespan_reporting::diagnostic::{Diagnostic, Label},
};
use std::path::Path;
use toml_edit::Document;

/// Context information available during validation.
pub struct ValidationContext<'a> {
    pub source: &'a str,
    pub path: &'a Path,
    pub document: &'a Document<String>,
}

/// Trait for types that can validate themselves.
///
/// This trait is typically derived using `#[derive(Validate)]` from
/// [`eden_config_derive`]. Manual implementations can provide custom validation logic.
pub trait Validate {
    /// Validates the value, returning a rendered diagnostic on failure.
    fn validate(&self, ctx: &ValidationContext<'_>) -> Result<(), RenderedDiagnostic>;
}

pub use eden_config_derive::Validate;

impl<T> Validate for Option<T>
where
    T: Validate,
{
    /// Validates the inner value if present.
    ///
    /// For `Option<T>`, validation only runs if the value is `Some`.
    /// `None` values always pass validation.
    fn validate(&self, ctx: &ValidationContext<'_>) -> Result<(), RenderedDiagnostic> {
        if let Some(inner) = self.as_ref() {
            inner.validate(ctx)?;
        }
        Ok(())
    }
}

/// Helper function to create a diagnostic error for a specific field.
///
/// This modularizes the common pattern of creating validation errors with
/// source location information.
pub(crate) fn create_field_error(
    ctx: &ValidationContext<'_>,
    path: &[&str],
    message: &str,
    mut modifier: impl FnMut(&mut Diagnostic<usize>),
) -> RenderedDiagnostic {
    let span = path
        .iter()
        .try_fold(ctx.document.as_item(), |item, key| item.get(key))
        .and_then(|item| item.span());

    let file_path = ctx.path.to_string_lossy();
    let renderer = Renderer::new().with_file(&file_path, ctx.source);

    let mut diagnostic = Diagnostic::error().with_message(message);
    if let Some(span) = span {
        let label = Label::primary(0usize, span);
        diagnostic = diagnostic.with_labels(vec![label]);
    }

    modifier(&mut diagnostic);
    renderer
        .render(diagnostic)
        .expect("rendering should succeed with valid file data")
}
