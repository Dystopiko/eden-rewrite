use bon::Builder;
use eden_config::types::setup::InitialSettings;
use eden_sqlx_sqlite::Transaction;
use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use sqlx::FromRow;
use thiserror::Error;
use twilight_model::id::{Id, marker::GuildMarker};

use crate::snowflake::Snowflake;

#[derive(Clone, Debug, FromRow)]
pub struct Settings {
    pub guild_id: Snowflake,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
    pub allow_guests: bool,
}

impl Settings {
    pub async fn find_or_insert(
        conn: &mut Transaction<'_>,
        guild_id: Id<GuildMarker>,
        setup: &InitialSettings,
    ) -> Result<(Settings, bool), Report<SettingsQueryError>> {
        let existing = sqlx::query_as::<_, Settings>(
            r#"
            SELECT * FROM settings
            WHERE guild_id = ?"#,
        )
        .bind(Snowflake::new(guild_id.cast()))
        .fetch_optional(&mut **conn)
        .await
        .change_context(SettingsQueryError)
        .attach("while checking whether specific settings exists")?;

        if let Some(existing) = existing {
            return Ok((existing, true));
        }

        NewSettings::new(guild_id, setup)
            .upsert(conn)
            .await
            .map(|v| (v, false))
    }
}

/// Error type representing a failure to query with the [`Settings`] table.
#[derive(Debug, Error)]
#[error("Failed to query settings table from the database")]
pub struct SettingsQueryError;

#[derive(Builder)]
pub struct NewSettings {
    pub guild_id: Snowflake,
    pub allow_guests: bool,
}

impl NewSettings {
    pub fn new(guild_id: Id<GuildMarker>, setup: &InitialSettings) -> NewSettings {
        NewSettings::builder()
            .guild_id(Snowflake::new(guild_id.cast()))
            .allow_guests(setup.allow_guests)
            .build()
    }

    pub async fn upsert(
        &self,
        conn: &mut Transaction<'_>,
    ) -> Result<Settings, Report<SettingsQueryError>> {
        sqlx::query_as::<_, Settings>(
            r#"
            INSERT INTO settings
            VALUES (?, ?, NULL, ?)
            ON CONFLICT (guild_id)
                DO UPDATE
                SET updated_at = excluded.created_at,
                    allow_guests = excluded.allow_guests
            RETURNING *
            "#,
        )
        .bind(Snowflake::new(self.guild_id.cast()))
        .bind(Timestamp::now())
        .bind(self.allow_guests)
        .fetch_one(&mut **conn)
        .await
        .change_context(SettingsQueryError)
        .attach("while trying to upsert settings")
    }
}

#[cfg(test)]
mod tests {
    use eden_config::types::setup::InitialSettings;
    use twilight_model::id::Id;

    use super::*;

    #[allow(clippy::needless_update)]
    #[tokio::test]
    async fn upsert_should_insert_new_settings() {
        let pool = crate::testing::setup().await;
        let mut conn = pool.begin().await.unwrap();

        let initial = InitialSettings::default();
        let settings = NewSettings::new(Id::new(1234), &initial)
            .upsert(&mut conn)
            .await
            .unwrap();

        assert_eq!(settings.guild_id.into_inner(), Id::new(1234));
        assert_eq!(settings.allow_guests, initial.allow_guests);
        assert!(settings.updated_at.is_none());

        conn.rollback().await.unwrap();
    }

    #[allow(clippy::needless_update)]
    #[tokio::test]
    async fn upsert_should_update_existing_settings() {
        let pool = crate::testing::setup().await;
        let mut conn = pool.begin().await.unwrap();

        let initial = InitialSettings::default();
        NewSettings::new(Id::new(1234), &initial)
            .upsert(&mut conn)
            .await
            .unwrap();

        // Upsert once more with different settings
        let settings = NewSettings::new(
            Id::new(1234),
            &InitialSettings {
                allow_guests: !initial.allow_guests,
                ..Default::default()
            },
        )
        .upsert(&mut conn)
        .await
        .unwrap();

        assert_eq!(settings.guild_id.into_inner(), Id::new(1234));
        assert_ne!(settings.allow_guests, initial.allow_guests);
        assert!(settings.updated_at.is_some());

        conn.rollback().await.unwrap();
    }

    #[allow(clippy::needless_update)]
    #[tokio::test]
    async fn find_or_insert_should_insert_when_not_exists() {
        let pool = crate::testing::setup().await;
        let mut conn = pool.begin().await.unwrap();

        let guild_id = Id::new(5678);
        let initial = InitialSettings {
            allow_guests: true,
            ..Default::default()
        };

        let (settings, existed) = Settings::find_or_insert(&mut conn, guild_id, &initial)
            .await
            .unwrap();

        assert!(!existed);
        assert_eq!(settings.guild_id, Snowflake::new(guild_id.cast()));
        assert!(settings.allow_guests);

        conn.rollback().await.unwrap();
    }

    #[allow(clippy::needless_update)]
    #[tokio::test]
    async fn find_or_insert_should_return_existing_when_exists() {
        let pool = crate::testing::setup().await;
        let mut conn = pool.begin().await.unwrap();

        let guild_id = Id::new(9999);
        let initial = InitialSettings {
            allow_guests: false,
            ..Default::default()
        };

        // Insert first time
        let (first_settings, existed) = Settings::find_or_insert(&mut conn, guild_id, &initial)
            .await
            .unwrap();
        assert!(!existed);

        // Find the same settings
        let different_settings = InitialSettings {
            allow_guests: true,
            ..Default::default()
        };
        let (second_settings, existed) =
            Settings::find_or_insert(&mut conn, guild_id, &different_settings)
                .await
                .unwrap();

        assert!(existed);
        assert_eq!(second_settings.guild_id, first_settings.guild_id);
        assert_eq!(second_settings.created_at, first_settings.created_at);
        // Should return existing settings, not the new ones from different_settings
        assert!(!second_settings.allow_guests);

        conn.rollback().await.unwrap();
    }
}
