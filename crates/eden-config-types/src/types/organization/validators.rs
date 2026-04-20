use eden_file_diagnostics::RenderedDiagnostic;

use crate::{context::SourceContext, types::Token};

/// Validates a Discord bot to make sure it is properly formatted.
pub fn validate_discord_token(
    token: &Token,
    ctx: &SourceContext<'_>,
) -> Result<(), RenderedDiagnostic> {
    let token_str = token.as_str();
    let has_valid_chars = token_str
        .chars()
        .all(|c| c.is_ascii() && !c.is_whitespace() && !c.is_control());

    if token_str.is_empty() || !has_valid_chars {
        let mut diagnostic = ctx.field_diagnostic(
            &["organization", "discord", "token"],
            "Invalid Discord bot token",
        );

        if token_str.is_empty() {
            diagnostic = diagnostic.with_note(TOKEN_CANNOT_BE_EMPTY);
        } else {
            diagnostic = diagnostic.with_note(TOKEN_ASCII_AND_NO_WHITESPACE);
        }

        diagnostic.with_note(GET_YOUR_TOKEN).emit()?;
    }

    Ok(())
}

const TOKEN_CANNOT_BE_EMPTY: &str = "Token cannot be empty";
const TOKEN_ASCII_AND_NO_WHITESPACE: &str =
    "Token must contain only ASCII characters with no whitespace";

const GET_YOUR_TOKEN: &str = "Get your bot token at: https://discord.com/developers/applications";
