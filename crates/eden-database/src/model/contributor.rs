use bon::Builder;
use eden_sqlx_sqlite::Transaction;
use eden_timestamp::Timestamp;
use sqlx::prelude::FromRow;
use twilight_model::id::{Id, marker::UserMarker};

use crate::snowflake::Snowflake;

#[derive(Clone, Debug, FromRow)]
pub struct Contributor {
    pub member_id: Snowflake,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}

#[derive(Builder)]
#[must_use = "this does not do anything unless it is called to execute"]
pub struct NewContributor {
    pub member_id: Id<UserMarker>,

    #[builder(default = Timestamp::now())]
    pub created_at: Timestamp,
}

impl NewContributor {
    pub async fn upsert(&self, conn: &mut Transaction<'_>) -> sqlx::Result<()> {
        sqlx::query("INSERT INTO contributors(member_id, created_at) VALUES (?, ?)")
            .bind(Snowflake::new(self.member_id.cast()))
            .bind(self.created_at)
            .execute(&mut **conn)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use claims::assert_ok;
    use twilight_model::id::Id;

    use crate::{
        model::{
            contributor::{Contributor, NewContributor},
            member::NewMember,
        },
        snowflake::Snowflake,
    };

    #[tokio::test]
    async fn should_upsert_contributor() {
        let pool = crate::testing::setup().await;
        let user_id = Id::new(123456);

        let mut conn = pool.begin().await.unwrap();
        let query = NewMember::builder()
            .discord_user_id(user_id)
            .name("Alice")
            .build();

        query.insert(&mut conn).await.unwrap();

        let query = NewContributor::builder().member_id(user_id).build();
        query.upsert(&mut conn).await.unwrap();

        let result = sqlx::query_as::<_, Contributor>(
            r#"SELECT * FROM contributors
            WHERE member_id = ?"#,
        )
        .bind(Snowflake::new(user_id.cast()))
        .fetch_one(&mut *conn)
        .await;

        assert_ok!(&result);

        let contributor = result.unwrap();
        assert_eq!(contributor.member_id, Snowflake::new(user_id.cast()));
    }
}
