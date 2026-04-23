use bon::Builder;
use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use sqlx::FromRow;
use thiserror::Error;
use twilight_model::id::{Id, marker::UserMarker};

use crate::snowflake::Snowflake;

#[derive(Clone, Debug, FromRow)]
pub struct Member {
    pub discord_user_id: Snowflake,
    pub joined_at: Timestamp,
    pub name: String,
    pub invited_by: Option<Snowflake>,
    pub updated_at: Timestamp,
}

impl Member {
    pub async fn find(
        discord_user_id: Id<UserMarker>,
        conn: &mut eden_postgres::Connection,
    ) -> Result<Member, Report<MemberQueryError>> {
        sqlx::query_as::<_, Member>(
            r#"
            SELECT * FROM members
            WHERE discord_user_id = $1"#,
        )
        .bind(Snowflake::new(discord_user_id.cast()))
        .fetch_one(conn)
        .await
        .change_context(MemberQueryError)
        .attach("while trying to find a member by discord user id")
    }
}

#[derive(Builder)]
pub struct NewMember<'a> {
    pub discord_user_id: Id<UserMarker>,
    #[builder(default = Timestamp::now())]
    pub joined_at: Timestamp,
    pub name: &'a str,
    pub invited_by: Option<Id<UserMarker>>,
}

#[derive(Debug, Error)]
#[error("could not query members table")]
pub struct MemberQueryError;

impl<'a> NewMember<'a> {
    pub async fn upsert(
        &self,
        conn: &mut eden_postgres::Connection,
    ) -> Result<(), Report<MemberQueryError>> {
        sqlx::query(
            r#"
            INSERT INTO members(
                discord_user_id, joined_at,
                name, invited_by
            )
            VALUES ($1, $2, $3, $4)
                ON CONFLICT (discord_user_id) DO UPDATE
                SET name = excluded.name,
                    invited_by = COALESCE(members.invited_by, excluded.invited_by),
                    updated_at = now()"#,
        )
        .bind(Snowflake::new(self.discord_user_id.cast()))
        .bind(self.joined_at)
        .bind(self.name)
        .bind(self.invited_by.map(Id::cast).map(Snowflake::new))
        .execute(conn)
        .await
        .change_context(MemberQueryError)
        .attach("while trying to upsert a member")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use claims::{assert_err, assert_ok};
    use eden_timestamp::Timestamp;
    use insta::assert_debug_snapshot;
    use twilight_model::id::Id;

    use crate::{
        model::member::{Member, NewMember},
        testing::TestPool,
    };

    #[tokio::test]
    async fn should_member_not_be_invited_by_itself() {
        let _guard = crate::testing::krate::setup();

        let pool = TestPool::with_migrations().await;
        let user_id = Id::new(12345);

        let mut conn = pool.acquire().await.unwrap();
        let query = NewMember::builder()
            .discord_user_id(user_id)
            .joined_at(Timestamp::from_secs(123456).unwrap())
            .name("john")
            .invited_by(user_id)
            .build();

        let result = query.upsert(&mut conn).await;
        assert_err!(&result);

        let error = result.unwrap_err();
        tracing::debug!(?error);

        let output = format!("{error:#?}");
        assert!(output.contains("members_should_not_invite_themselves"));
    }

    #[tokio::test]
    async fn should_upsert_member() {
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

        let query = NewMember::builder()
            .discord_user_id(user_id)
            .name("jane")
            .build();

        let result = query.upsert(&mut conn).await;
        assert_ok!(&result);

        let mut member = Member::find(user_id, &mut conn).await.unwrap();
        member.updated_at = Timestamp::from_secs(0).unwrap();

        assert_debug_snapshot!(member);
    }

    #[tokio::test]
    async fn should_insert_member() {
        let _guard = crate::testing::krate::setup();

        let pool = TestPool::with_migrations().await;
        let user_id = Id::new(12345);

        let mut conn = pool.acquire().await.unwrap();
        let result = NewMember::builder()
            .discord_user_id(user_id)
            .joined_at(Timestamp::from_secs(123456).unwrap())
            .name("john")
            .build()
            .upsert(&mut conn)
            .await;

        assert_ok!(&result);

        let mut member = Member::find(user_id, &mut conn).await.unwrap();
        member.updated_at = Timestamp::from_secs(0).unwrap();

        assert_debug_snapshot!(member);
    }
}
