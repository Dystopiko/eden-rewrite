use bon::Builder;
use eden_sqlx_sqlite::Transaction;
use eden_timestamp::Timestamp;
use sqlx::prelude::FromRow;
use twilight_model::id::{Id, marker::UserMarker};

use crate::snowflake::Snowflake;

#[derive(Clone, Debug, FromRow)]
pub struct Staff {
    pub member_id: Snowflake,
    pub joined_at: Timestamp,
    pub updated_at: Option<Timestamp>,
    pub admin: bool,
}

#[derive(Builder)]
#[must_use = "this does not do anything unless it is called to execute"]
pub struct NewStaff {
    pub member_id: Id<UserMarker>,
    #[builder(default = Timestamp::now())]
    pub joined_at: Timestamp,
    #[builder(default = false)]
    pub admin: bool,
}

impl NewStaff {
    pub async fn upsert(&self, conn: &mut Transaction<'_>) -> sqlx::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO staffs (member_id, joined_at, admin)
            VALUES (?, ?, ?)
            ON CONFLICT (member_id)
                DO UPDATE
                SET admin = excluded.admin,
                    updated_at = current_timestamp
            RETURNING *"#,
        )
        .bind(Snowflake::new(self.member_id.cast()))
        .bind(self.joined_at)
        .bind(self.admin)
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
            member::NewMember,
            staff::{NewStaff, Staff},
        },
        snowflake::Snowflake,
    };

    #[tokio::test]
    async fn should_upsert_staff() {
        let pool = crate::testing::setup().await;
        let user_id = Id::new(123456);

        let mut conn = pool.begin().await.unwrap();
        let query = NewMember::builder()
            .discord_user_id(user_id)
            .name("Alice")
            .build();

        query.insert(&mut conn).await.unwrap();

        let query = NewStaff::builder().member_id(user_id).build();
        query.upsert(&mut conn).await.unwrap();

        let result = sqlx::query_as::<_, Staff>(
            r#"SELECT * FROM staffs
            WHERE member_id = ?"#,
        )
        .bind(Snowflake::new(user_id.cast()))
        .fetch_one(&mut *conn)
        .await;

        assert_ok!(&result);

        let staff = result.unwrap();
        assert_eq!(staff.member_id, Snowflake::new(user_id.cast()));
        assert!(!staff.admin);
    }
}
