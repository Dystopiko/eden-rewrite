use eden_file_diagnostics::RenderedDiagnostic;
use error_stack::Report;
use toml_edit::DocumentMut;

pub fn migrate_v2_to_v3(document: &mut DocumentMut) -> Result<(), Report<RenderedDiagnostic>> {
    if document
        .get("gateway")
        .and_then(|v| v.as_table_like())
        .is_some()
    {
        gateway_tls_fields_to_own_table(document);
    }

    if document
        .get("sentry")
        .and_then(|v| v.as_table_like())
        .is_some()
    {
        renames_in_sentry(document);
    }

    if document
        .get("bot")
        .and_then(|v| v.as_table_like())
        .is_some()
    {
        arrange_fields_from_bot(document);
        document.remove("bot");
    }

    Ok(())
}

fn renames_in_sentry(document: &mut DocumentMut) -> Option<()> {
    let sentry = document
        .get_mut("sentry")
        .and_then(|v| v.as_table_like_mut())?;

    // Rename env to environment
    if sentry.contains_key("env")
        && let Some(env) = sentry.remove("env")
    {
        sentry.insert("environment", env);
        sentry.remove("env");
    }

    Some(())
}

fn gateway_tls_fields_to_own_table(document: &mut DocumentMut) -> Option<()> {
    let gateway = document
        .get_mut("gateway")
        .and_then(|v| v.as_table_like_mut())?;

    // Migrate gateway.tls_* -> gateway.tls.*
    let tls_cert_pem = gateway
        .get("tls_cert_pem")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let tls_private_key_pem = gateway
        .get("tls_private_key_pem")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());

    let tls = gateway
        .entry("tls")
        .or_insert_with(toml_edit::table)
        .as_table_like_mut()?;

    if let Some(value) = tls_cert_pem {
        tls.insert("cert_file", toml_edit::value(value));
    }

    if let Some(value) = tls_private_key_pem {
        tls.insert("priv_key_file", toml_edit::value(value));
    }

    gateway.remove("tls_cert_pem");
    gateway.remove("tls_private_key_pem");
    Some(())
}

fn arrange_fields_from_bot(document: &mut DocumentMut) -> Option<()> {
    // Extract values first to avoid borrow conflicts
    let token = document
        .get("bot")
        .and_then(|v| v.as_table_like())
        .and_then(|bot| bot.get("token"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let guild_id = document
        .get("bot")
        .and_then(|v| v.as_table_like())
        .and_then(|bot| bot.get("primary_guild"))
        .and_then(|v| v.as_table_like())
        .and_then(|guild| guild.get("id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Then, we can get raw tables from a document
    let discord = document
        .entry("organization")
        .or_insert_with(toml_edit::table)
        .as_table_like_mut()?
        .entry("discord")
        .or_insert_with(toml_edit::table)
        .as_table_like_mut()?;

    // Migrate bot.token -> organization.discord.token
    if let Some(token) = token {
        discord.insert("token", toml_edit::value(token));
    }

    // Migrate bot.primary_guild.id -> organization.discord.guild_id
    if let Some(guild_id) = guild_id {
        discord.insert("guild_id", toml_edit::value(guild_id));
    }

    document.remove("bot");
    Some(())
}
