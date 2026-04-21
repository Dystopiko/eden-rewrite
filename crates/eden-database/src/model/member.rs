use bon::Builder;
use eden_sqlx_sqlite::{Connection, Transaction};
use eden_timestamp::Timestamp;
use sqlx::prelude::FromRow;
use twilight_model::id::{Id, marker::UserMarker};

use crate::snowflake::Snowflake;

/// Represents a member of an organization's Discord server.
#[derive(Clone, Debug, FromRow)]
pub struct Member {
    pub discord_user_id: Snowflake,
    pub joined_at: Timestamp,
    pub name: String,
    pub updated_at: Option<Timestamp>,
    pub invited_by: Option<Snowflake>,
    pub nickname: Option<String>,
}

impl Member {
    pub async fn find(
        conn: &mut Connection,
        discord_user_id: Id<UserMarker>,
    ) -> sqlx::Result<Self> {
        sqlx::query_as::<_, Member>(
            r#"
            SELECT * FROM members
            WHERE discord_user_id = ?"#,
        )
        .bind(Snowflake::new(discord_user_id.cast()))
        .fetch_one(conn)
        .await
    }
}

#[derive(Builder)]
#[must_use = "this does not do anything unless it is called to execute"]
pub struct NewMember<'a> {
    pub discord_user_id: Id<UserMarker>,
    #[builder(default = Timestamp::now())]
    pub joined_at: Timestamp,
    pub name: &'a str,
    pub invited_by: Option<Id<UserMarker>>,
    pub nickname: Option<&'a str>,
}

impl<'a> NewMember<'a> {
    pub fn new(member: &'a twilight_model::guild::Member) -> Self {
        let joined_at = member.joined_at.map(Timestamp::from_twilight);

        Self::builder()
            .discord_user_id(member.user.id)
            .maybe_joined_at(joined_at)
            .name(&member.user.name)
            .maybe_nickname(member.nick.as_deref())
            .build()
    }

    pub async fn insert(&self, conn: &mut Transaction<'_>) -> sqlx::Result<Member> {
        sqlx::query_as::<_, Member>(
            r#"
            INSERT INTO members (
                discord_user_id, joined_at, name,
                invited_by, nickname
            )
            VALUES (?, ?, ?, ?, ?)
            RETURNING *"#,
        )
        .bind(Snowflake::new(self.discord_user_id.cast()))
        .bind(self.joined_at)
        .bind(self.name)
        .bind(self.invited_by.map(|v| Snowflake::new(v.cast())))
        .bind(self.nickname)
        .fetch_one(&mut **conn)
        .await
    }

    pub async fn upsert(&self, conn: &mut Transaction<'_>) -> sqlx::Result<Member> {
        sqlx::query_as::<_, Member>(
            r#"
            INSERT INTO members (
                discord_user_id, joined_at, name,
                invited_by, nickname
            )
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT (discord_user_id)
                DO UPDATE
                SET name = excluded.name,
                    updated_at = current_timestamp,
                    invited_by = excluded.invited_by,
                    nickname = excluded.nickname
            RETURNING *"#,
        )
        .bind(Snowflake::new(self.discord_user_id.cast()))
        .bind(self.joined_at)
        .bind(self.name)
        .bind(self.invited_by.map(|v| Snowflake::new(v.cast())))
        .fetch_one(&mut **conn)
        .await
    }
}

#[cfg(test)]
mod tests {
    use claims::{assert_err, assert_none};
    use eden_sqlx_sqlite::{
        SqliteErrorType,
        error::{QueryResultExt, SqliteResultExt},
    };
    use error_stack::ResultExt;
    use twilight_model::id::Id;

    use crate::{
        model::member::{Member, NewMember},
        snowflake::Snowflake,
    };

    #[tokio::test]
    async fn should_upsert_member() {
        let pool = crate::testing::setup().await;
        let user_id = Id::new(123456);

        let mut conn = pool.begin().await.unwrap();
        let query = NewMember::builder()
            .discord_user_id(user_id)
            .name("Alice")
            .build();

        let prev = query.upsert(&mut conn).await.unwrap();
        assert_eq!(prev.discord_user_id, Snowflake::new(user_id.cast()));
        assert_eq!(prev.name, "Alice");

        let query = NewMember::builder()
            .discord_user_id(user_id)
            .name("Bob")
            .build();

        let next = query.upsert(&mut conn).await.unwrap();
        assert_eq!(next.discord_user_id, prev.discord_user_id);
        assert_eq!(next.invited_by, prev.invited_by);
        assert_eq!(next.joined_at, prev.joined_at);
    }

    #[tokio::test]
    async fn should_insert_member() {
        let pool = crate::testing::setup().await;
        let user_id = Id::new(987654321);

        let mut conn = pool.begin().await.unwrap();
        let query = NewMember::builder()
            .discord_user_id(user_id)
            .name("Bob")
            .build();

        let member = query.insert(&mut conn).await.unwrap();
        assert_eq!(member.discord_user_id, Snowflake::new(user_id.cast()));
        assert_eq!(member.name, "Bob");
    }

    #[tokio::test]
    async fn should_throw_if_member_exists_while_inserting() {
        let pool = crate::testing::setup().await;
        let user_id = Id::new(987654321);

        let mut conn = pool.begin().await.unwrap();
        let query = NewMember::builder()
            .discord_user_id(user_id)
            .name("Bob")
            .build();

        query.insert(&mut conn).await.unwrap();

        let query = NewMember::builder()
            .discord_user_id(user_id)
            .name("Alice")
            .build();

        let result = query.insert(&mut conn).await;
        assert_err!(&result);

        let kind = result
            .attach("")
            .sqlite_error_type()
            .expect("should provide SQLite error type");

        assert_eq!(
            kind,
            SqliteErrorType::UniqueViolation(
                "UNIQUE constraint failed: members.discord_user_id".into()
            )
        );
    }

    #[tokio::test]
    async fn should_find_member() {
        let pool = crate::testing::setup().await;
        let user_id = Id::new(777777777);

        let mut conn = pool.begin().await.unwrap();
        let member = NewMember::builder()
            .discord_user_id(user_id)
            .name("Diana")
            .build();

        member.insert(&mut conn).await.unwrap();

        let found = Member::find(&mut conn, user_id).await.unwrap();
        assert_eq!(found.discord_user_id, Snowflake::new(user_id.cast()));
        assert_eq!(found.name, "Diana");

        let non_existent_id = Id::new(999999999);
        let result = crate::model::member::Member::find(&mut conn, non_existent_id).await;
        assert_err!(&result);
        assert_none!(result.attach("").optional().unwrap());
    }
}
