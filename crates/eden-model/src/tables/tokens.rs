use bitflags::bitflags;
use bon::Builder;
use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use sqlx::{FromRow, Type};
use thiserror::Error;
use twilight_model::id::{Id, marker::UserMarker};
use uuid::Uuid;

use crate::snowflake::Snowflake;

#[derive(Clone, Debug, FromRow)]
pub struct Token {
    pub id: Uuid,
    pub created_at: Timestamp,
    pub last_used_at: Option<Timestamp>,
    pub name: String,
    pub hashed: String,

    /// Only populated for user tokens.
    pub member_id: Option<Snowflake>,

    #[sqlx(rename = "type")]
    pub kind: TokenType,

    /// Only populated for user tokens.
    pub permissions: Option<PermissionScope>,
    pub authorized_by: String,
    pub expires_at: Option<Timestamp>,
    pub revoked: bool,
}

impl Token {
    pub async fn find_by_hashed(
        conn: &mut eden_postgres::Connection,
        hashed: impl AsRef<[u8]>,
    ) -> Result<Self, Report<TokenQueryError>> {
        sqlx::query_as::<_, Token>(
            r#"
            SELECT * FROM tokens
            WHERE hashed = $1
                AND revoked = FALSE
                AND (expires_at IS NULL OR expires_at > now())"#,
        )
        .bind(hashed.as_ref())
        .fetch_one(conn)
        .await
        .change_context(TokenQueryError)
        .attach("while trying to find token by hashed token")
    }

    pub async fn update_last_used_at(
        &mut self,
        conn: &mut eden_postgres::Connection,
    ) -> Result<(), Report<TokenQueryError>> {
        let last_used_at = sqlx::query_scalar::<_, Timestamp>(
            r#"
            UPDATE tokens
            SET last_used_at = now()
            WHERE id = $1
                AND revoked = FALSE
                AND (expires_at IS NULL OR expires_at > now())
            RETURNING last_used_at"#,
        )
        .bind(self.id)
        .fetch_one(conn)
        .await
        .change_context(TokenQueryError)
        .attach("while trying to update last_used_at to a token")?;

        self.last_used_at = Some(last_used_at);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Type)]
#[sqlx(type_name = "varchar", rename_all = "kebab-case")]
pub enum TokenType {
    /// Token is dedicated for authorized users.
    User,

    /// Token is dedicated for a Minecraft server with EdenMC
    McServer,
}

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
    pub struct PermissionScope: u64 {
        /// Grant all permissions for the authorized user.
        const ALL = 1 << 0;

        /// Whether the authorized users can edit global Eden settings.
        const EDIT_SETTINGS = 1 << 1;

        /// Whether the authorized users can link Minecraft accounts
        /// manually for the organization members.
        const LINK_MINECRAFT_ACCOUNTS = 1 << 2;
    }
}

impl PermissionScope {
    #[must_use]
    pub const fn has(&self, requirements: Self) -> bool {
        if self.contains(Self::ALL) {
            true
        } else {
            self.contains(requirements)
        }
    }
}

#[derive(Builder)]
pub struct NewToken {
    #[builder(default = Uuid::new_v4())]
    pub id: Uuid,

    #[builder(into)]
    pub name: String,

    #[builder(into)]
    pub hashed: String,
    pub member_id: Option<Id<UserMarker>>,

    #[builder(setters(name = "set_permissions", vis = ""))]
    pub permissions: Option<PermissionScope>,

    #[builder(into)]
    pub authorized_by: String,
    pub expires_at: Option<Timestamp>,
}

#[derive(Debug, Error)]
#[error("could not query tokens table")]
pub struct TokenQueryError;

impl NewToken {
    pub async fn insert(
        &self,
        conn: &mut eden_postgres::Connection,
    ) -> Result<Token, Report<TokenQueryError>> {
        let kind = if self.permissions.is_some() {
            TokenType::User
        } else {
            TokenType::McServer
        };

        sqlx::query_as::<_, Token>(
            r#"
            INSERT INTO tokens (
                id, name, member_id, permissions, hashed,
                type, authorized_by, expires_at
            )
            VALUES ($1, $2, $3, COALESCE($4, 0), $5, $6, $7, $8)
            RETURNING *"#,
        )
        .bind(self.id)
        .bind(&self.name)
        .bind(self.member_id.map(|v| Snowflake::new(v.cast())))
        .bind(self.permissions)
        .bind(&self.hashed)
        .bind(kind)
        .bind(&self.authorized_by)
        .bind(self.expires_at)
        .fetch_one(conn)
        .await
        .change_context(TokenQueryError)
        .attach("while trying to insert a new token")
    }
}

impl<S> NewTokenBuilder<S>
where
    S: new_token_builder::State,
{
    pub fn permissions(
        self,
        scope: PermissionScope,
    ) -> NewTokenBuilder<new_token_builder::SetPermissions<S>>
    where
        S::MemberId: new_token_builder::IsSet,
        S::Permissions: new_token_builder::IsUnset,
    {
        self.set_permissions(scope)
    }
}

impl<'row> sqlx::Decode<'row, sqlx::Postgres> for PermissionScope
where
    i64: sqlx::Decode<'row, sqlx::Postgres>,
{
    fn decode(
        value: <sqlx::Postgres as sqlx::Database>::ValueRef<'row>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let value = u64::try_from(i64::decode(value)?).unwrap_or(0);
        Ok(PermissionScope::from_bits_truncate(value))
    }
}
impl<'row> sqlx::Encode<'row, sqlx::Postgres> for PermissionScope
where
    i64: sqlx::Encode<'row, sqlx::Postgres>,
{
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Postgres as sqlx::Database>::ArgumentBuffer<'row>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        (self.0.bits() as i64).encode_by_ref(buf)
    }
}

impl sqlx::Type<sqlx::Postgres> for PermissionScope
where
    i64: sqlx::Type<sqlx::Postgres>,
{
    fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
        <i64 as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

#[cfg(test)]
mod tests {
    use claims::assert_ok;
    use eden_timestamp::Timestamp;
    use insta::assert_debug_snapshot;
    use twilight_model::id::Id;
    use uuid::Uuid;

    use crate::tables::{
        member::NewMember,
        tokens::{NewToken, PermissionScope, Token},
    };

    const HASHED_CODE: &str = "0424974c68530290458c8d58674e2637f65abc127057957d7b3acbd24c208f93";

    #[sqlx::test(migrations = "../../migrations")]
    async fn should_update_last_used_at(pool: sqlx::PgPool) {
        let _guard = crate::testing::krate::setup();

        let mut conn = pool.begin().await.unwrap();
        NewToken::builder()
            .name("test_token")
            .authorized_by("test_function")
            .hashed(HASHED_CODE)
            .build()
            .insert(&mut conn)
            .await
            .unwrap();

        let mut token = Token::find_by_hashed(&mut conn, HASHED_CODE).await.unwrap();
        assert_ok!(token.update_last_used_at(&mut conn).await);
        assert!(token.last_used_at.is_some());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn should_find_token_by_hashed(pool: sqlx::PgPool) {
        let _guard = crate::testing::krate::setup();

        let mut conn = pool.begin().await.unwrap();
        NewToken::builder()
            .name("test_token")
            .authorized_by("test_function")
            .hashed(HASHED_CODE)
            .build()
            .insert(&mut conn)
            .await
            .unwrap();

        assert_ok!(Token::find_by_hashed(&mut conn, HASHED_CODE).await);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn should_insert_new_user_token(pool: sqlx::PgPool) {
        let _guard = crate::testing::krate::setup();
        let member_id = Id::new(273534239310479360);

        let mut conn = pool.begin().await.unwrap();
        NewMember::builder()
            .discord_user_id(member_id)
            .name("steve")
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        let query = NewToken::builder()
            .name("test_token")
            .authorized_by("test_function")
            .member_id(member_id)
            .hashed(HASHED_CODE)
            .permissions(PermissionScope::all())
            .build();

        let mut token = assert_ok!(query.insert(&mut conn).await);
        token.id = Uuid::nil();
        token.created_at = Timestamp::from_secs(0).unwrap();

        assert_debug_snapshot!(token);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn should_insert_new_mod_token(pool: sqlx::PgPool) {
        let _guard = crate::testing::krate::setup();

        let mut conn = pool.begin().await.unwrap();
        let query = NewToken::builder()
            .name("test_token")
            .authorized_by("test_function")
            .hashed(HASHED_CODE)
            .build();

        let mut token = assert_ok!(query.insert(&mut conn).await);
        token.id = Uuid::nil();
        token.created_at = Timestamp::from_secs(0).unwrap();

        assert_debug_snapshot!(token);
    }

    #[test]
    fn should_grant_all_permissions_for_all_permission() {
        let permissions = PermissionScope::ALL;
        assert!(permissions.has(PermissionScope::EDIT_SETTINGS));
    }
}
