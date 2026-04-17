use eden_file_diagnostics::RenderedDiagnostic;
use std::path::Path;
use toml_edit::Document;

/// Context information available during validation
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
    /// It only validates the value if it is [`Some`].
    fn validate(&self, ctx: &ValidationContext<'_>) -> Result<(), RenderedDiagnostic> {
        if let Some(inner) = self.as_ref() {
            inner.validate(ctx)?;
        }
        Ok(())
    }
}
