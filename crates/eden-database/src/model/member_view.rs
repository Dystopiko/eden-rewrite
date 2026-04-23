use bitflags::bitflags;
use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use sqlx::Row;
use thiserror::Error;
use twilight_model::id::{Id, marker::UserMarker};

use crate::snowflake::Snowflake;

#[derive(Clone, Debug)]
pub struct MemberView {
    pub discord_user_id: Snowflake,
    pub joined_at: Timestamp,
    pub name: String,
    pub flags: MemberFlags,
    pub inviter: Option<Inviter>,
}

#[derive(Clone, Debug)]
pub struct Inviter {
    pub discord_user_id: Snowflake,
    pub name: String,
    pub flags: MemberFlags,
}

impl MemberView {
    pub async fn find(
        discord_user_id: Id<UserMarker>,
        conn: &mut eden_postgres::Connection,
    ) -> Result<MemberView, Report<MemberViewQueryError>> {
        sqlx::query_as::<_, MemberView>("SELECT * FROM member_view WHERE discord_user_id = $1")
            .bind(Snowflake::new(discord_user_id.cast()))
            .fetch_one(conn)
            .await
            .change_context(MemberViewQueryError)
            .attach("while trying to find a member from a member view")
    }
}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for MemberView {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        let discord_user_id = row.try_get("discord_user_id")?;
        let joined_at = row.try_get("joined_at")?;
        let name = row.try_get("name")?;
        let flags = row.try_get("flags")?;
        let inviter = Self::extract_inviter(row)?;

        Ok(Self {
            discord_user_id,
            joined_at,
            name,
            flags,
            inviter,
        })
    }
}

impl MemberView {
    fn extract_inviter(row: &sqlx::postgres::PgRow) -> Result<Option<Inviter>, sqlx::Error> {
        let invited_by: Option<Snowflake> = row.try_get("invited_by")?;
        let inviter_name: Option<String> = row.try_get("inviter_name")?;
        let inviter_flags: Option<MemberFlags> = row.try_get("inviter_flags")?;

        Ok(match (invited_by, inviter_name, inviter_flags) {
            (Some(discord_user_id), Some(name), Some(flags)) => Some(Inviter {
                discord_user_id,
                name,
                flags,
            }),
            _ => None,
        })
    }
}

#[derive(Debug, Error)]
#[error("could not query member_view table")]
pub struct MemberViewQueryError;

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
    pub struct MemberFlags: u64 {
        const REGULAR = 0;
        const CONTRIBUTOR = 1 << 0;
        const STAFF = 1 << 1;
        const ADMINISTRATOR = 1 << 2;
    }
}

impl MemberFlags {
    #[must_use]
    pub const fn is_regular(&self) -> bool {
        self.is_empty()
    }

    #[must_use]
    pub const fn is_contributor(&self) -> bool {
        self.intersects(Self::CONTRIBUTOR)
    }

    #[must_use]
    pub const fn is_staff(&self) -> bool {
        self.intersects(Self::STAFF)
    }

    #[must_use]
    pub const fn is_admin(&self) -> bool {
        self.intersects(Self::ADMINISTRATOR)
    }
}

impl<'row> sqlx::Decode<'row, sqlx::Postgres> for MemberFlags
where
    i32: sqlx::Decode<'row, sqlx::Postgres>,
{
    fn decode(
        value: <sqlx::Postgres as sqlx::Database>::ValueRef<'row>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let value = i32::decode(value)? as u64;
        Ok(MemberFlags::from_bits_truncate(value))
    }
}

impl<'row> sqlx::Encode<'row, sqlx::Postgres> for MemberFlags
where
    i32: sqlx::Encode<'row, sqlx::Postgres>,
{
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Postgres as sqlx::Database>::ArgumentBuffer<'row>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        (self.0.bits() as i32).encode_by_ref(buf)
    }
}

impl sqlx::Type<sqlx::Postgres> for MemberFlags
where
    i32: sqlx::Type<sqlx::Postgres>,
{
    fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
        <i32 as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

#[cfg(test)]
mod tests {
    use claims::assert_ok;
    use eden_timestamp::Timestamp;
    use insta::assert_debug_snapshot;
    use twilight_model::id::Id;

    use crate::{
        model::{
            contributor::NewContributor, member::NewMember, member_view::MemberView,
            staff::NewStaff,
        },
        testing::TestPool,
    };

    #[tokio::test]
    async fn should_include_an_inviter() {
        let _guard = crate::testing::krate::setup();

        let pool = TestPool::with_migrations().await;

        let alice_user_id = Id::new(12345);
        let bob_user_id = Id::new(12346);

        let mut conn = pool.acquire().await.unwrap();
        NewMember::builder()
            .discord_user_id(alice_user_id)
            .joined_at(Timestamp::from_secs(123456).unwrap())
            .name("alice")
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        NewMember::builder()
            .discord_user_id(bob_user_id)
            .joined_at(Timestamp::from_secs(123456).unwrap())
            .name("bob")
            .invited_by(alice_user_id)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        let result = MemberView::find(bob_user_id, &mut conn).await;
        assert_ok!(&result);

        let view = result.unwrap();
        assert_debug_snapshot!(view);
    }

    #[tokio::test]
    async fn flags_should_work_with_members_with_multiple_roles() {
        let _guard = crate::testing::krate::setup();

        let pool = TestPool::with_migrations().await;
        let user_id = Id::new(12345);

        let mut conn = pool.acquire().await.unwrap();
        NewMember::builder()
            .discord_user_id(user_id)
            .joined_at(Timestamp::from_secs(123456).unwrap())
            .name("john")
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        NewContributor::builder()
            .member_id(user_id)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        NewStaff::builder()
            .member_id(user_id)
            .admin(true)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        let result = MemberView::find(user_id, &mut conn).await;
        assert_ok!(&result);

        let view = result.unwrap();
        assert_debug_snapshot!(view);
    }

    #[tokio::test]
    async fn should_include_staff_flag_if_member_is_admin() {
        let _guard = crate::testing::krate::setup();

        let pool = TestPool::with_migrations().await;
        let user_id = Id::new(12345);

        let mut conn = pool.acquire().await.unwrap();
        NewMember::builder()
            .discord_user_id(user_id)
            .joined_at(Timestamp::from_secs(123456).unwrap())
            .name("john")
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        NewStaff::builder()
            .member_id(user_id)
            .admin(true)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        let result = MemberView::find(user_id, &mut conn).await;
        assert_ok!(&result);

        let view = result.unwrap();
        assert!(view.flags.is_admin());
        assert_debug_snapshot!(view);
    }

    #[tokio::test]
    async fn should_include_staff_flag_if_member_is_staff() {
        let _guard = crate::testing::krate::setup();

        let pool = TestPool::with_migrations().await;
        let user_id = Id::new(12345);

        let mut conn = pool.acquire().await.unwrap();
        NewMember::builder()
            .discord_user_id(user_id)
            .joined_at(Timestamp::from_secs(123456).unwrap())
            .name("john")
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        NewStaff::builder()
            .member_id(user_id)
            .admin(false)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        let result = MemberView::find(user_id, &mut conn).await;
        assert_ok!(&result);

        let view = result.unwrap();
        assert!(view.flags.is_staff());
        assert_debug_snapshot!(view);
    }

    #[tokio::test]
    async fn should_include_contributor_flag_if_member_is_contributor() {
        let _guard = crate::testing::krate::setup();

        let pool = TestPool::with_migrations().await;
        let user_id = Id::new(12345);

        let mut conn = pool.acquire().await.unwrap();
        NewMember::builder()
            .discord_user_id(user_id)
            .joined_at(Timestamp::from_secs(123456).unwrap())
            .name("john")
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        NewContributor::builder()
            .member_id(user_id)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        let result = MemberView::find(user_id, &mut conn).await;
        assert_ok!(&result);

        let view = result.unwrap();
        assert!(view.flags.is_contributor());
        assert_debug_snapshot!(view);
    }

    #[tokio::test]
    async fn should_find_member() {
        let _guard = crate::testing::krate::setup();

        let pool = TestPool::with_migrations().await;
        let user_id = Id::new(12345);

        let mut conn = pool.acquire().await.unwrap();
        NewMember::builder()
            .discord_user_id(user_id)
            .joined_at(Timestamp::from_secs(123456).unwrap())
            .name("john")
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        let result = MemberView::find(user_id, &mut conn).await;
        assert_ok!(&result);

        let member = result.unwrap();
        assert!(member.flags.is_regular());
        assert_debug_snapshot!(member);
    }
}
