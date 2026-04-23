use bon::Builder;
use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use sqlx::FromRow;
use thiserror::Error;
use twilight_model::id::{Id, marker::UserMarker};
use uuid::Uuid;

use crate::{mc_edition::McEdition, snowflake::Snowflake};

#[derive(Clone, Debug, FromRow)]
pub struct LinkedMcAccount {
    pub member_id: Snowflake,
    pub uuid: Uuid,
    pub linked_at: Timestamp,
    pub username: String,
    pub edition: McEdition,
}

impl LinkedMcAccount {
    pub async fn from_mc_uuid(
        uuid: Uuid,
        conn: &mut eden_postgres::Connection,
    ) -> Result<LinkedMcAccount, Report<QueryError>> {
        sqlx::query_as::<_, Self>(
            r#"
            SELECT * from linked_mc_accounts
            WHERE uuid = $1"#,
        )
        .bind(uuid)
        .fetch_one(conn)
        .await
        .change_context(QueryError)
        .attach("while trying to find a linked minecraft account from uuid")
    }
}

#[derive(Builder)]
pub struct LinkMcAccount<'a> {
    pub member_id: Id<UserMarker>,
    pub uuid: Uuid,
    pub username: &'a str,
    pub edition: McEdition,
}

impl LinkMcAccount<'_> {
    pub async fn insert(
        &self,
        conn: &mut eden_postgres::Connection,
    ) -> Result<LinkedMcAccount, Report<QueryError>> {
        sqlx::query_as::<_, LinkedMcAccount>(
            r#"
            INSERT INTO linked_mc_accounts (
                member_id, uuid, username, edition
            )
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
        )
        .bind(Snowflake::new(self.member_id.cast()))
        .bind(self.uuid)
        .bind(self.username)
        .bind(self.edition)
        .fetch_one(conn)
        .await
        .change_context(QueryError)
        .attach("while trying to insert a linked minecraft account")
    }
}

#[derive(Debug, Error)]
#[error("could not query linked_mc_accounts table")]
pub struct QueryError;

#[cfg(test)]
mod tests {
    use claims::assert_ok;
    use eden_timestamp::Timestamp;
    use insta::assert_debug_snapshot;
    use std::str::FromStr;
    use twilight_model::id::Id;
    use uuid::Uuid;

    use crate::{
        model::{
            linked_mc_account::{LinkMcAccount, LinkedMcAccount, McEdition},
            member::NewMember,
        },
        testing::TestPool,
    };

    #[tokio::test]
    async fn should_find_linked_mc_account_by_mc_uuid() {
        let _guard = crate::testing::krate::setup();

        let pool = TestPool::with_migrations().await;
        let user_id = Id::new(12345);

        let mut conn = pool.acquire().await.unwrap();
        NewMember::builder()
            .discord_user_id(user_id)
            .name("john")
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        let uuid = Uuid::from_str("e8b6254a-f39b-4a68-9744-708fd82eef54").unwrap();
        LinkMcAccount::builder()
            .member_id(user_id)
            .uuid(uuid)
            .username("john")
            .edition(McEdition::Java)
            .build()
            .insert(&mut conn)
            .await
            .unwrap();

        let result = LinkedMcAccount::from_mc_uuid(uuid, &mut conn).await;
        assert_ok!(&result);

        let mut linked = result.unwrap();
        linked.linked_at = Timestamp::from_secs(123456).unwrap();

        assert_debug_snapshot!(linked);
    }

    #[tokio::test]
    async fn should_link_mc_account() {
        let _guard = crate::testing::krate::setup();

        let pool = TestPool::with_migrations().await;
        let user_id = Id::new(12345);

        let mut conn = pool.acquire().await.unwrap();
        NewMember::builder()
            .discord_user_id(user_id)
            .name("john")
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        let uuid = Uuid::from_str("e8b6254a-f39b-4a68-9744-708fd82eef54").unwrap();
        let result = LinkMcAccount::builder()
            .member_id(user_id)
            .uuid(uuid)
            .username("john")
            .edition(McEdition::Java)
            .build()
            .insert(&mut conn)
            .await;

        assert_ok!(&result);

        let mut linked = result.unwrap();
        linked.linked_at = Timestamp::from_secs(123456).unwrap();

        assert_debug_snapshot!(linked);
    }
}
