use bon::Builder;
use eden_sqlx_sqlite::{Connection, Transaction};
use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use sqlx::FromRow;
use thiserror::Error;
use twilight_model::id::{Id, marker::UserMarker};
use uuid::Uuid;

use crate::{minecraft::McAccountType, snowflake::Snowflake};

/// A Minecraft account linked to a primary guild member.
#[derive(Debug, Clone, FromRow)]
pub struct McAccount {
    pub id: i32,
    pub linked_at: Timestamp,
    pub member_id: Snowflake,
    pub uuid: Uuid,
    pub username: String,
    #[sqlx(rename = "type")]
    pub kind: McAccountType,
}

impl McAccount {
    pub async fn find_by_uuid(
        conn: &mut Connection,
        uuid: Uuid,
    ) -> Result<Self, Report<McAccountQueryError>> {
        sqlx::query_as::<_, McAccount>("SELECT * FROM minecraft_accounts WHERE uuid = ?")
            .bind(uuid)
            .fetch_one(conn)
            .await
            .change_context(McAccountQueryError)
            .attach("while trying to find minecraft account by uuid")
    }

    pub async fn get_all(
        conn: &mut Connection,
        member_id: Id<UserMarker>,
    ) -> Result<Vec<Self>, Report<McAccountQueryError>> {
        sqlx::query_as::<_, McAccount>(
            r#"
            SELECT * FROM minecraft_accounts
            WHERE member_id = ?"#,
        )
        .bind(Snowflake::new(member_id.cast()))
        .fetch_all(conn)
        .await
        .change_context(McAccountQueryError)
        .attach("while trying to fetching all minecraft accounts from a member")
    }
}

/// Error type representing a failure to query with the [`McAccount`] table.
#[derive(Debug, Error)]
#[error("Failed to query minecraft account table from the database")]
pub struct McAccountQueryError;

#[derive(Builder)]
pub struct NewMcAccount<'a> {
    pub member_id: Id<UserMarker>,
    pub uuid: Uuid,
    pub username: &'a str,
    pub account_type: McAccountType,
}

impl<'a> NewMcAccount<'a> {
    pub async fn create(
        &self,
        conn: &mut Transaction<'_>,
    ) -> Result<McAccount, Report<McAccountQueryError>> {
        sqlx::query_as::<_, McAccount>(
            r#"
            INSERT INTO minecraft_accounts (member_id, uuid, username, "type")
            VALUES (?, ?, ?, ?)
            RETURNING *"#,
        )
        .bind(Snowflake::new(self.member_id.cast()))
        .bind(self.uuid)
        .bind(self.username)
        .bind(self.account_type)
        .fetch_one(&mut **conn)
        .await
        .change_context(McAccountQueryError)
        .attach("while trying to create minecraft account")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::member::NewMember;
    use std::str::FromStr;

    #[tokio::test]
    async fn create_should_insert_new_minecraft_account() {
        let pool = crate::testing::setup().await;
        let mut tx = pool.begin().await.unwrap();

        let member_id = Id::new(123456789);
        let member = NewMember::builder()
            .discord_user_id(member_id)
            .name("Steve")
            .build();
        member.insert(&mut tx).await.unwrap();

        let mc_uuid = Uuid::from_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let mc_account = NewMcAccount::builder()
            .member_id(member_id)
            .uuid(mc_uuid)
            .username("Steve123")
            .account_type(McAccountType::Java)
            .build();

        let result = mc_account.create(&mut tx).await.unwrap();

        assert_eq!(result.uuid, mc_uuid);
        assert_eq!(result.username, "Steve123");
        assert_eq!(result.member_id, Snowflake::new(member_id.cast()));
        assert_eq!(result.kind, McAccountType::Java);

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn find_by_uuid_should_return_minecraft_account() {
        let pool = crate::testing::setup().await;
        let mut tx = pool.begin().await.unwrap();

        let member_id = Id::new(987654321);
        let member = NewMember::builder()
            .discord_user_id(member_id)
            .name("Alex")
            .build();
        member.insert(&mut tx).await.unwrap();

        let mc_uuid = Uuid::from_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();
        let mc_account = NewMcAccount::builder()
            .member_id(member_id)
            .uuid(mc_uuid)
            .username("Alex456")
            .account_type(McAccountType::Bedrock)
            .build();

        mc_account.create(&mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let mut conn = pool.acquire().await.unwrap();
        let found = McAccount::find_by_uuid(&mut conn, mc_uuid).await.unwrap();

        assert_eq!(found.uuid, mc_uuid);
        assert_eq!(found.username, "Alex456");
        assert_eq!(found.kind, McAccountType::Bedrock);
    }

    #[tokio::test]
    async fn find_by_uuid_should_fail_when_account_not_exists() {
        let pool = crate::testing::setup().await;
        let mut conn = pool.acquire().await.unwrap();

        let non_existent_uuid = Uuid::from_str("00000000-0000-0000-0000-000000000000").unwrap();
        let result = McAccount::find_by_uuid(&mut conn, non_existent_uuid).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn get_all_should_return_all_accounts_for_member() {
        let pool = crate::testing::setup().await;
        let mut tx = pool.begin().await.unwrap();

        let member_id = Id::new(555555555);
        let member = NewMember::builder()
            .discord_user_id(member_id)
            .name("MultiAccount")
            .build();
        member.insert(&mut tx).await.unwrap();

        // Create multiple accounts for the same member
        let java_uuid = Uuid::from_str("550e8400-e29b-41d4-a716-446655440001").unwrap();
        let bedrock_uuid = Uuid::from_str("550e8400-e29b-41d4-a716-446655440002").unwrap();

        NewMcAccount::builder()
            .member_id(member_id)
            .uuid(java_uuid)
            .username("JavaPlayer")
            .account_type(McAccountType::Java)
            .build()
            .create(&mut tx)
            .await
            .unwrap();

        NewMcAccount::builder()
            .member_id(member_id)
            .uuid(bedrock_uuid)
            .username("BedrockPlayer")
            .account_type(McAccountType::Bedrock)
            .build()
            .create(&mut tx)
            .await
            .unwrap();

        tx.commit().await.unwrap();

        let mut conn = pool.acquire().await.unwrap();
        let accounts = McAccount::get_all(&mut conn, member_id).await.unwrap();

        assert_eq!(accounts.len(), 2);
        assert!(accounts.iter().any(|a| a.uuid == java_uuid));
        assert!(accounts.iter().any(|a| a.uuid == bedrock_uuid));
    }

    #[tokio::test]
    async fn get_all_should_return_empty_when_no_accounts() {
        let pool = crate::testing::setup().await;
        let mut tx = pool.begin().await.unwrap();

        let member_id = Id::new(777777777);
        let member = NewMember::builder()
            .discord_user_id(member_id)
            .name("NoAccounts")
            .build();
        member.insert(&mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let mut conn = pool.acquire().await.unwrap();
        let accounts = McAccount::get_all(&mut conn, member_id).await.unwrap();

        assert_eq!(accounts.len(), 0);
    }
}
