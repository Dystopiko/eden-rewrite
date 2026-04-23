use bon::Builder;
use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use sqlx::FromRow;
use std::net::IpAddr;
use thiserror::Error;
use twilight_model::id::{Id, marker::UserMarker};
use uuid::Uuid;

use crate::{
    mc_edition::McEdition, model::linked_mc_account::LinkedMcAccount, snowflake::Snowflake,
};

#[derive(Clone, Debug, FromRow)]
pub struct McLoginEvent {
    pub id: Uuid,
    pub player_uuid: Uuid,
    pub created_at: Timestamp,
    pub ip_address: IpAddr,
    pub username: Option<String>,
    pub edition: McEdition,
    pub member_id: Option<Snowflake>,
}

#[derive(Builder)]
pub struct NewMcLoginEvent<'a> {
    player_uuid: Uuid,
    #[builder(default = Timestamp::now())]
    created_at: Timestamp,
    ip_address: IpAddr,
    edition: McEdition,

    #[builder(setters(vis = ""))]
    username: Option<&'a str>,
    #[builder(setters(vis = ""))]
    member_id: Option<Id<UserMarker>>,
}

impl<'a> NewMcLoginEvent<'a> {
    pub fn from_linked(account: &'a LinkedMcAccount) -> BuilderFromLinked<'a> {
        NewMcLoginEvent::builder()
            .player_uuid(account.uuid)
            .username(&account.username)
            .edition(account.edition)
            .member_id(account.member_id.cast())
    }

    pub async fn insert(
        &self,
        conn: &mut eden_postgres::Connection,
    ) -> Result<McLoginEvent, Report<McLoginEventQueryError>> {
        sqlx::query_as::<_, McLoginEvent>(
            r#"
            INSERT INTO mc_login_events (
                id, player_uuid, created_at, ip_address,
                username, edition, member_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *"#,
        )
        .bind(Uuid::new_v4())
        .bind(self.player_uuid)
        .bind(self.created_at)
        .bind(self.ip_address)
        .bind(self.username)
        .bind(self.edition)
        .bind(self.member_id.map(Id::cast).map(Snowflake::new))
        .fetch_one(conn)
        .await
        .change_context(McLoginEventQueryError)
        .attach("while trying to insert a new minecraft login event")
    }
}

#[derive(Debug, Error)]
#[error("could not query mc_login_events table")]
pub struct McLoginEventQueryError;

use self::private::BuilderFromLinked;
mod private {
    use super::{NewMcLoginEventBuilder, new_mc_login_event_builder::*};

    pub type BuilderFromLinked<'a> =
        NewMcLoginEventBuilder<'a, SetMemberId<SetEdition<SetUsername<SetPlayerUuid<Empty>>>>>;
}

#[cfg(test)]
mod tests {
    use claims::assert_ok;
    use eden_timestamp::Timestamp;
    use insta::assert_debug_snapshot;
    use std::{
        net::{IpAddr, Ipv4Addr},
        str::FromStr,
    };
    use twilight_model::id::Id;
    use uuid::Uuid;

    use crate::{
        mc_edition::McEdition,
        model::mc_login_event::NewMcLoginEvent,
        testing::{TestPool, krate::member_with_linked_mc_account},
    };

    #[tokio::test]
    async fn should_insert_mc_login_event_from_guest() {
        let _guard = crate::testing::krate::setup();

        let pool = TestPool::with_migrations().await;

        let mut conn = pool.acquire().await.unwrap();
        let query = NewMcLoginEvent::builder()
            .edition(McEdition::Bedrock)
            .ip_address(IpAddr::V4(Ipv4Addr::LOCALHOST))
            .player_uuid(Uuid::from_str("e8b6254a-f39b-4a68-9744-708fd82eef54").unwrap())
            .build();

        let result = query.insert(&mut conn).await;
        assert_ok!(&result);

        let mut event = result.unwrap();
        event.id = Uuid::nil();
        event.created_at = Timestamp::from_secs(123456).unwrap();

        assert_debug_snapshot!(event);
    }

    #[tokio::test]
    async fn should_insert_mc_login_event_from_mc_account() {
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

        let result = NewMcLoginEvent::from_linked(&account)
            .ip_address(IpAddr::V4(Ipv4Addr::LOCALHOST))
            .build()
            .insert(&mut conn)
            .await;

        assert_ok!(&result);

        let mut event = result.unwrap();
        event.id = Uuid::from_str("e8b6254a-f39b-4a68-9744-708fd82eef54").unwrap();
        event.created_at = Timestamp::from_secs(123456).unwrap();
        event.player_uuid = Uuid::nil();

        assert_debug_snapshot!(event);
    }
}
