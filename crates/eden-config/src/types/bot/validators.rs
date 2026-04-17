use crate::validation::ValidationContext;
use eden_file_diagnostics::{
    RenderedDiagnostic, Renderer,
    codespan_reporting::diagnostic::{Diagnostic, Label},
};

use super::Token;

/// Validates a Discord bot to make sure it is properly formatted.
pub fn validate_token(
    token: &Token,
    ctx: &ValidationContext<'_>,
) -> Result<(), RenderedDiagnostic> {
    let token_str = token.as_str();
    let has_valid_chars = token_str
        .chars()
        .all(|c| c.is_ascii() && !c.is_whitespace() && !c.is_control());

    if token_str.is_empty() || !has_valid_chars {
        let span = ctx
            .document
            .get("bot")
            .and_then(|v| v.get("token"))
            .and_then(|v| v.span());

        let path = ctx.path.to_string_lossy();
        let renderer = Renderer::new().with_file(&path, ctx.source);

        let mut diagnostic = Diagnostic::error().with_message("Invalid Discord token");
        if let Some(span) = span {
            let label = Label::primary(0usize, span);
            diagnostic = diagnostic.with_labels(vec![label]);
        }

        let diagnostic = renderer
            .render(diagnostic)
            .expect("rendering should succeed with valid file data");

        return Err(diagnostic);
    }

    Ok(())
}
