use eden_file_diagnostics::RenderedDiagnostic;

use crate::context::SourceContext;

/// Trait for types that can validate themselves.
///
/// This trait is typically derived using `#[derive(Validate)]` from
/// [`eden_config_derive`]. Manual implementations can provide custom validation logic.
pub trait Validate {
    /// Validates the value, returning a rendered diagnostic on failure.
    fn validate(&self, ctx: &SourceContext<'_>) -> Result<(), RenderedDiagnostic>;
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
    fn validate(&self, ctx: &SourceContext<'_>) -> Result<(), RenderedDiagnostic> {
        if let Some(inner) = self.as_ref() {
            inner.validate(ctx)?;
        }
        Ok(())
    }
}
