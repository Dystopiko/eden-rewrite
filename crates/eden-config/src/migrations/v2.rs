use eden_file_diagnostics::RenderedDiagnostic;
use error_stack::Report;
use toml_edit::DocumentMut;

use crate::migrations::MigrationResult;

pub fn migrate_v2_to_v3(
    document: &mut DocumentMut,
    result: &mut MigrationResult,
) -> Result<(), Report<RenderedDiagnostic>> {
    let sentry = document
        .get_mut("sentry")
        .and_then(|v| v.as_table_like_mut());

    if let Some(sentry) = sentry {
        // Rename env to environment
        if sentry.contains_key("env")
            && let Some(env) = sentry.remove("env")
        {
            sentry.insert("environment", env);
            result
                .warnings
                .push("changed from `sentry.env` to `sentry.environment`");
        }
    }

    Ok(())
}
