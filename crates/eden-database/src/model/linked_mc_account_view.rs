use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use sqlx::Row;
use thiserror::Error;
use uuid::Uuid;

use crate::{mc_edition::McEdition, model::member_view::MemberView};

#[derive(Clone, Debug)]
pub struct LinkedMcAccountView {
    pub member: MemberView,
    pub uuid: Uuid,
    pub linked_at: Timestamp,
    pub username: String,
    pub edition: McEdition,
}

impl LinkedMcAccountView {
    pub async fn from_mc_uuid(
        uuid: Uuid,
        conn: &mut eden_postgres::Connection,
    ) -> Result<Self, Report<ViewQueryError>> {
        sqlx::query_as::<_, Self>(
            r#"
            SELECT * FROM linked_mc_account_view
            WHERE uuid = $1"#,
        )
        .bind(uuid)
        .fetch_one(conn)
        .await
        .change_context(ViewQueryError)
        .attach("while trying to find a linked minecraft account from view")
    }
}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for LinkedMcAccountView {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        let member: MemberView = MemberView::from_row(row)?;
        Ok(Self {
            member,
            uuid: row.try_get("uuid")?,
            linked_at: row.try_get("linked_at")?,
            username: row.try_get("username")?,
            edition: row.try_get("edition")?,
        })
    }
}

#[derive(Debug, Error)]
#[error("could not query linked_mc_account_view table")]
pub struct ViewQueryError;

#[cfg(test)]
mod tests {
    use claims::assert_ok;
    use eden_timestamp::Timestamp;
    use insta::assert_debug_snapshot;
    use twilight_model::id::Id;
    use uuid::Uuid;

    use crate::{
        model::linked_mc_account_view::LinkedMcAccountView,
        testing::{TestPool, krate::member_with_linked_mc_account},
    };

    #[tokio::test]
    async fn should_find_linked_mc_account_view_by_mc_uuid() {
        let _guard = crate::testing::krate::setup();

        let pool = TestPool::with_migrations().await;
        let user_id = Id::new(12345);

        let mut conn = pool.acquire().await.unwrap();
        let account = member_with_linked_mc_account()
            .name("john")
            .discord_user_id(user_id)
            .mc_username("john1")
            .conn(&mut conn)
            .call()
            .await;

        let result = LinkedMcAccountView::from_mc_uuid(account.uuid, &mut conn).await;
        assert_ok!(&result);

        let mut view = result.unwrap();
        view.uuid = Uuid::nil();
        view.member.joined_at = Timestamp::from_secs(1234567).unwrap();
        view.linked_at = Timestamp::from_secs(1234567).unwrap();

        assert_debug_snapshot!(view);
    }
}
