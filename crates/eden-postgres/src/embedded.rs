//! Embedded PostgreSQL server management for testing and development.
use eden_config::types::database::Embedded;
use error_stack::{Report, ResultExt};
use postgresql_embedded::{PostgreSQL, Settings, SettingsBuilder, Status, VersionReq};
use thiserror::Error;
use tokio::sync::OnceCell;

/// Error that occurs when starting or configuring the embedded PostgreSQL server.
#[derive(Debug, Error)]
#[error("Failed to start embedded PostgreSQL server")]
pub struct EmbedError;

static SERVER: OnceCell<PostgreSQL> = OnceCell::const_new();

/// Starts or retrieves the shared test PostgreSQL server.
///
/// # Panics
///
/// Panics if the PostgreSQL server fails to start. This is intentional
/// for tests, as a failed database setup should halt the test suite.
#[must_use]
pub async fn start_for_tests() -> &'static PostgreSQL {
    SERVER
        .get_or_init(|| async {
            tracing::warn!("CACHE MISS!");

            let version = VersionReq::parse("17")
                .expect("17.x.x should be a valid semver version requirement");

            let settings = SettingsBuilder::new()
                .temporary(true)
                .version(version)
                .build();

            start_inner(settings)
                .await
                .expect("Failed to start PostgreSQL server for tests")
        })
        .await
}

/// Starts an embedded PostgreSQL server with custom configuration.
///
/// This function creates and starts a new PostgreSQL server instance based on
/// the provided configuration. Unlike [`start_for_tests`], this creates a new
/// server instance each time it's called.
pub async fn start(config: &Embedded) -> Result<PostgreSQL, Report<EmbedError>> {
    let settings = build_settings(config);
    start_inner(settings).await
}

fn build_settings(config: &Embedded) -> Settings {
    let mut builder = SettingsBuilder::new()
        .configuration(config.configuration.clone())
        .host(&config.host)
        .port(config.port)
        .username(&config.username)
        .password(config.password.as_str())
        .version(config.version.clone())
        .temporary(config.temporary);

    if let Some(ref data_dir) = config.data_dir {
        builder = builder.data_dir(data_dir);
    }

    if let Some(ref password_file) = config.password_file {
        builder = builder.password_file(password_file);
    }

    if let Some(ref installation_dir) = config.installation_dir {
        builder = builder.installation_dir(installation_dir);
    }

    if let Some(trust) = config.trust_installation_dir {
        builder = builder.trust_installation_dir(trust);
    }

    if let Some(ref socket_dir) = config.socket_dir {
        builder = builder.socket_dir(socket_dir);
    }

    builder.build()
}

#[tracing::instrument(
    skip_all,
    name = "start",
    fields(
        data_dir = ?settings.data_dir,
        releases_url = %settings.releases_url,
        temporary = ?settings.temporary,
        version = %settings.version,
    )
)]
async fn start_inner(settings: Settings) -> Result<PostgreSQL, Report<EmbedError>> {
    ensure_data_directory(&settings).await?;

    let mut psql = PostgreSQL::new(settings);
    if psql.status() == Status::Started {
        handle_stale_server(&mut psql).await;
    }

    setup_postgres_cluster(&mut psql).await?;

    psql.start()
        .await
        .change_context(EmbedError)
        .attach("Failed to start PostgreSQL server")?;

    tracing::info!("PostgreSQL server started successfully");
    Ok(psql)
}

async fn ensure_data_directory(settings: &Settings) -> Result<(), Report<EmbedError>> {
    if !settings.data_dir.exists() {
        tracing::debug!(
            data_dir = ?settings.data_dir,
            "Creating PostgreSQL data directory"
        );

        tokio::fs::create_dir_all(&settings.data_dir)
            .await
            .change_context(EmbedError)
            .attach("Failed to create data directory")
            .attach(format!("Path: {:?}", settings.data_dir))?;
    }
    Ok(())
}

async fn handle_stale_server(psql: &mut PostgreSQL) {
    tracing::warn!("PostgreSQL server appears to be already running. Attempting to stop it...");

    if let Err(error) = psql.stop().await {
        tracing::warn!(?error, "Failed to stop existing PostgreSQL server");

        // Try to remove stale PID file
        let pid_file = psql.settings().data_dir.join("postmaster.pid");
        if pid_file.exists() {
            if let Err(error) = tokio::fs::remove_file(&pid_file).await {
                tracing::warn!(?error, ?pid_file, "failed to remove stale PID file");
            } else {
                tracing::debug!("removed stale PID file");
            }
        }
    } else {
        tracing::debug!("successfully stopped existing server");
    }
}

async fn setup_postgres_cluster(psql: &mut PostgreSQL) -> Result<(), Report<EmbedError>> {
    match psql.setup().await {
        Ok(()) => {
            tracing::debug!("PostgreSQL database cluster initialized");
            Ok(())
        }
        Err(error) => {
            use postgresql_embedded::Error;

            // DatabaseInitializationError is sometimes thrown even when the
            // database is properly initialized (e.g., directory already exists)
            if matches!(error, Error::DatabaseInitializationError(..)) {
                tracing::warn!(
                    status = ?psql.status(),
                    "Database initialization reported an error, but may already be initialized"
                );
                Ok(())
            } else {
                Err(Report::new(error))
                    .change_context(EmbedError)
                    .attach("Failed to initialize PostgreSQL database cluster")
            }
        }
    }
}
