use bon::Builder;
use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use thiserror::Error;
use twilight_model::id::{Id, marker::UserMarker};

use crate::snowflake::Snowflake;

#[derive(Builder)]
pub struct NewContributor {
    member_id: Id<UserMarker>,
    #[builder(default = Timestamp::now())]
    joined_at: Timestamp,
}

impl NewContributor {
    pub async fn upsert(
        &self,
        conn: &mut eden_postgres::Connection,
    ) -> Result<(), Report<ContributorQueryError>> {
        sqlx::query(
            r#"
            INSERT INTO contributors(member_id, joined_at)
            VALUES ($1, $2)
                ON CONFLICT (member_id) DO UPDATE
                SET updated_at = now()"#,
        )
        .bind(Snowflake::new(self.member_id.cast()))
        .bind(self.joined_at)
        .execute(conn)
        .await
        .change_context(ContributorQueryError)
        .attach("while trying to upsert a contributor")?;

        Ok(())
    }
}

#[derive(Debug, Error)]
#[error("could not query contributors table")]
pub struct ContributorQueryError;

#[cfg(test)]
mod tests {
    use claims::{assert_ok, assert_some};
    use eden_timestamp::Timestamp;
    use twilight_model::id::Id;

    use crate::{
        model::{contributor::NewContributor, member::NewMember},
        snowflake::Snowflake,
        testing::TestPool,
    };

    #[tokio::test]
    async fn should_upsert_contributor() {
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

        NewContributor::builder()
            .member_id(user_id)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        let query = NewContributor::builder().member_id(user_id).build();
        let result = query.upsert(&mut conn).await;
        assert_ok!(&result);

        let result = sqlx::query_scalar::<_, Timestamp>(
            r#"SELECT updated_at
            FROM contributors
            WHERE member_id = $1"#,
        )
        .bind(Snowflake::new(user_id.cast()))
        .fetch_optional(&mut *conn)
        .await;

        assert_ok!(&result);

        let updated_at = result.unwrap();
        assert_some!(&updated_at);
    }

    #[tokio::test]
    async fn should_insert_contributor() {
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

        let query = NewContributor::builder().member_id(user_id).build();
        let result = query.upsert(&mut conn).await;
        assert_ok!(&result);
    }
}
