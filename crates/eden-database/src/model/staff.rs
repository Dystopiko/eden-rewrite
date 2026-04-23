use bon::Builder;
use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use thiserror::Error;
use twilight_model::id::{Id, marker::UserMarker};

use crate::snowflake::Snowflake;

#[derive(Builder)]
pub struct NewStaff {
    member_id: Id<UserMarker>,
    #[builder(default = Timestamp::now())]
    joined_at: Timestamp,
    #[builder(default = false)]
    admin: bool,
}

impl NewStaff {
    pub async fn upsert(
        &self,
        conn: &mut eden_postgres::Connection,
    ) -> Result<(), Report<StaffQueryError>> {
        sqlx::query(
            r#"
            INSERT INTO staff(member_id, joined_at, admin)
            VALUES ($1, $2, $3)
                ON CONFLICT (member_id) DO UPDATE
                SET updated_at = now(),
                    admin = COALESCE(excluded.admin, staff.admin)"#,
        )
        .bind(Snowflake::new(self.member_id.cast()))
        .bind(self.joined_at)
        .bind(self.admin)
        .execute(conn)
        .await
        .change_context(StaffQueryError)
        .attach("while trying to upsert a staff")?;

        Ok(())
    }
}

#[derive(Debug, Error)]
#[error("could not query staff table")]
pub struct StaffQueryError;

#[cfg(test)]
mod tests {
    use claims::{assert_ok, assert_some};
    use eden_timestamp::Timestamp;
    use twilight_model::id::Id;

    use crate::{
        model::{member::NewMember, staff::NewStaff},
        snowflake::Snowflake,
        testing::TestPool,
    };

    #[tokio::test]
    async fn should_upsert_staff() {
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

        NewStaff::builder()
            .member_id(user_id)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        let query = NewStaff::builder().member_id(user_id).build();
        let result = query.upsert(&mut conn).await;
        assert_ok!(&result);

        let result = sqlx::query_scalar::<_, Timestamp>(
            r#"SELECT updated_at
            FROM staff
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

        let query = NewStaff::builder().member_id(user_id).build();
        let result = query.upsert(&mut conn).await;
        assert_ok!(&result);
    }
}
