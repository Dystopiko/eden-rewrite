use eden_file_diagnostics::RenderedDiagnostic;

use super::Token;
use crate::validation::{ValidationContext, create_field_error};

const TOKEN_CANNOT_BE_EMPTY: &str = "Token cannot be empty";
const TOKEN_ASCII_AND_NO_WHITESPACE: &str =
    "Token must contain only ASCII characters with no whitespace";

const GET_YOUR_TOKEN: &str = "Get your bot token at: https://discord.com/developers/applications";

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
        return Err(create_field_error(
            ctx,
            &["bot", "token"],
            "Invalid Discord bot token",
            |diagnostic| {
                if token_str.is_empty() {
                    diagnostic.notes.push(TOKEN_CANNOT_BE_EMPTY.into());
                } else {
                    diagnostic.notes.push(TOKEN_ASCII_AND_NO_WHITESPACE.into());
                }
                diagnostic.notes.push(GET_YOUR_TOKEN.into());
            },
        ));
    }

    Ok(())
}
