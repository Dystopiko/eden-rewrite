use eden_file_diagnostics::RenderedDiagnostic;
use std::path::Path;

use crate::validation::{ValidationContext, create_field_error};

pub fn check_for_tls_cert_file(
    path: &Path,
    ctx: &ValidationContext<'_>,
) -> Result<(), RenderedDiagnostic> {
    check_tls_pem_field("cert_file", path, ctx)
}

pub fn check_for_tls_priv_key_file(
    path: &Path,
    ctx: &ValidationContext<'_>,
) -> Result<(), RenderedDiagnostic> {
    check_tls_pem_field("priv_key_file", path, ctx)
}

fn check_tls_pem_field(
    field: &str,
    path: &Path,
    ctx: &ValidationContext<'_>,
) -> Result<(), RenderedDiagnostic> {
    // Treat these values as valid since portions of the string may not be valid UTF-8.
    let is_empty = path.to_str().map(|v| v.is_empty()).unwrap_or(false);
    if is_empty {
        return Err(create_field_error(
            ctx,
            &["gateway", "tls", field],
            format_args!("Missing `{field}` path"),
            |_| {},
        ));
    }
    Ok(())
}
