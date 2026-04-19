use eden_file_diagnostics::{
    RenderedDiagnostic,
    codespan_reporting::diagnostic::{Label, LabelStyle},
};
use std::str::FromStr;
use toml_edit::DocumentMut;

use crate::{context::SourceContext, types::organization::minecraft::PerkId};

pub fn migrate_to_v3(
    ctx: &SourceContext,
    document: &mut DocumentMut,
) -> Result<(), RenderedDiagnostic> {
    if ctx
        .document
        .get("minecraft")
        .and_then(|v| v.as_table_like())
        .is_some()
    {
        minecraft(ctx, document)?;
        document.remove("minecraft");
    }

    if ctx
        .document
        .get("bot")
        .and_then(|v| v.as_table_like())
        .is_some()
    {
        arrange_fields_from_bot(ctx, document)?;
        document.remove("bot");
    }

    if ctx
        .document
        .get("sentry")
        .and_then(|v| v.as_table_like())
        .is_some()
    {
        renames_in_sentry(document);
    }

    if ctx
        .document
        .get("gateway")
        .and_then(|v| v.as_table_like())
        .is_some()
    {
        gateway_tls_fields_to_own_table(document);
    }

    Ok(())
}

fn arrange_fields_from_bot(
    ctx: &SourceContext,
    document: &mut DocumentMut,
) -> Result<(), RenderedDiagnostic> {
    // Extract values first to avoid borrow conflicts
    let Some(original) = ctx.document.get("bot").and_then(|v| v.as_table_like()) else {
        return Ok(());
    };

    // Then, we can get raw tables from a document
    let discord = document
        .entry("organization")
        .or_insert_with(toml_edit::table)
        .as_table_like_mut()
        .ok_or_else(|| {
            ctx.field_diagnostic(&["organization"], "unexpected organization is not a table")
                .into_diagnostic()
        })?
        .entry("discord")
        .or_insert_with(toml_edit::table)
        .as_table_like_mut()
        .ok_or_else(|| {
            ctx.field_diagnostic(
                &["organization", "discord"],
                "unexpected organization.discord is not a table",
            )
            .into_diagnostic()
        })?;

    let primary_guild = original
        .get("primary_guild")
        .and_then(|v| v.as_table_like());

    // Migrate bot.token -> organization.discord.token
    if let Some(token) = original.get("token").and_then(|v| v.as_str()) {
        discord.insert("token", toml_edit::value(token));
    }

    // Migrate bot.primary_guild.id -> organization.discord.guild_id
    if let Some(guild_id) = primary_guild
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
    {
        discord.insert("guild_id", toml_edit::value(guild_id));
    }

    // No other migrations needed for the swearing police except for:
    // - bot.swearing_police -> organization.discord.swearing_police
    // - bot.primary_guild.bad_words -> organization.discord.swearing_police.bad_words
    if let Some(table) = original
        .get("swearing_police")
        .and_then(|v| v.is_table_like().then(|| v.clone()))
    {
        discord.insert("swearing_police", table);
    }

    if let Some(dest) = discord
        .entry("swearing_police")
        .or_insert_with(toml_edit::table)
        .as_table_like_mut()
        && let Some(item) = primary_guild
            .and_then(|v| v.get("bad_words"))
            .and_then(|v| v.is_array().then(|| v.clone()))
    {
        dest.insert("bad_words", item);
    }

    Ok(())
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

// Username identifiers are not supported anymore, so we need to throw an error to the user.
fn minecraft(ctx: &SourceContext, document: &mut DocumentMut) -> Result<(), RenderedDiagnostic> {
    let perks = ctx
        .document
        .get("minecraft")
        .and_then(|v| v.as_table_like())
        .and_then(|v| v.get("perks"))
        .and_then(|v| v.as_table_like());

    let Some(perks) = perks else { return Ok(()) };

    let minecraft = document
        .entry("organization")
        .or_insert_with(toml_edit::table)
        .as_table_like_mut()
        .ok_or_else(|| {
            ctx.field_diagnostic(&["organization"], "unexpected organization is not a table")
                .into_diagnostic()
        })?
        .entry("minecraft")
        .or_insert_with(toml_edit::table)
        .as_table_like_mut()
        .ok_or_else(|| {
            ctx.field_diagnostic(
                &["organization", "minecraft"],
                "unexpected organization.minecraft is not a table",
            )
            .into_diagnostic()
        })?;

    let perks_dest = minecraft
        .entry("perks")
        .or_insert_with(toml_edit::table)
        .as_table_like_mut()
        .ok_or_else(|| {
            ctx.field_diagnostic(
                &["organization", "minecraft", "perks"],
                "unexpected organization.minecraft.perks is not a table",
            )
            .into_diagnostic()
        })?;

    for (key, value) in perks.iter() {
        PerkId::from_str(key).map_err(|e| {
            let span = perks
                .key(key)
                .and_then(|v| v.span())
                .expect("original document is referred");

            let label = Label::new(LabelStyle::Secondary, 0usize, span)
                .with_message("Username identifiers are not supported. Use their Discord IDs or Minecraft UUIDs instead.");

            ctx.field_diagnostic(&["minecraft", "perks"], e)
                .with_label(label)
                .into_diagnostic()
        })?;
        perks_dest.insert(key, value.clone());
    }

    Ok(())
}
