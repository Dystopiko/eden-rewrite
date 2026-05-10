use bon::Builder;
use eden_config::types::setup::InitialSettings;
use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use sqlx::FromRow;
use thiserror::Error;
use twilight_model::id::{Id, marker::GuildMarker};

use crate::snowflake::Snowflake;

#[derive(Clone, Debug, FromRow)]
pub struct Settings {
    pub org_guild_id: Snowflake,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
    pub allow_guests: bool,
}

impl Settings {
    pub async fn find(
        org_guild_id: Id<GuildMarker>,
        conn: &mut eden_postgres::Connection,
    ) -> Result<Settings, Report<SettingsQueryError>> {
        sqlx::query_as::<_, Settings>("SELECT * FROM settings WHERE org_guild_id = $1")
            .bind(Snowflake::new(org_guild_id.cast()))
            .fetch_one(conn)
            .await
            .change_context(SettingsQueryError)
            .attach("while trying to find settings from an organization's Discord guild ID")
    }
}

#[derive(Builder)]
pub struct NewSettings {
    pub org_guild_id: Id<GuildMarker>,
    pub allow_guests: bool,
}

type SetState<S> = new_settings_builder::SetAllowGuests<S>;

impl<S: new_settings_builder::State> NewSettingsBuilder<S> {
    pub fn use_initial_settings(self, settings: &InitialSettings) -> NewSettingsBuilder<SetState<S>>
    where
        S::AllowGuests: new_settings_builder::IsUnset,
    {
        self.allow_guests(settings.allow_guests)
    }
}

#[derive(Debug, Error)]
#[error("could not query settings table")]
pub struct SettingsQueryError;

impl NewSettings {
    pub async fn upsert(
        &self,
        conn: &mut eden_postgres::Transaction<'_>,
    ) -> Result<Settings, Report<SettingsQueryError>> {
        sqlx::query_as(
            r#"
            INSERT INTO settings
            VALUES ($1, now(), NULL, $2)
            ON CONFLICT (org_guild_id)
                DO UPDATE
                SET updated_at = excluded.created_at,
                    allow_guests = excluded.allow_guests
            RETURNING *
        "#,
        )
        .bind(Snowflake::new(self.org_guild_id.cast()))
        .bind(self.allow_guests)
        .fetch_one(&mut **conn)
        .await
        .change_context(SettingsQueryError)
        .attach("while trying to upsert settings")
    }
}

#[cfg(test)]
mod tests {
    use claims::assert_ok;
    use eden_timestamp::Timestamp;
    use insta::assert_debug_snapshot;
    use twilight_model::id::Id;

    use crate::tables::settings::{NewSettings, Settings};

    #[sqlx::test(migrations = "../../migrations")]
    async fn should_upsert_settings(pool: sqlx::PgPool) {
        let _guard = crate::testing::krate::setup();
        let org_guild_id = Id::new(1234567);

        let mut conn = pool.begin().await.unwrap();
        NewSettings::builder()
            .org_guild_id(org_guild_id)
            .allow_guests(true)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        let query = NewSettings::builder()
            .org_guild_id(org_guild_id)
            .allow_guests(false)
            .build();

        let result = query.upsert(&mut conn).await;
        assert_ok!(&result);

        let mut settings = Settings::find(org_guild_id, &mut conn).await.unwrap();
        settings.created_at = Timestamp::from_secs(1234567).unwrap();
        settings.updated_at = Timestamp::from_secs(3234567).ok();

        assert_debug_snapshot!(settings);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn should_insert_settings(pool: sqlx::PgPool) {
        let _guard = crate::testing::krate::setup();
        let org_guild_id = Id::new(1234567);

        let mut conn = pool.begin().await.unwrap();
        let query = NewSettings::builder()
            .org_guild_id(org_guild_id)
            .allow_guests(true)
            .build();

        let result = query.upsert(&mut conn).await;
        assert_ok!(&result);

        let mut settings = Settings::find(org_guild_id, &mut conn).await.unwrap();
        settings.created_at = Timestamp::from_secs(1234567).unwrap();

        assert_debug_snapshot!(settings);
    }
}
