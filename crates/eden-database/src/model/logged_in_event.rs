use bon::Builder;
use eden_sqlx_sqlite::Transaction;
use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use std::net::IpAddr;
use thiserror::Error;
use twilight_model::id::{Id, marker::UserMarker};
use uuid::Uuid;

use crate::{minecraft::McAccountType, snowflake::Snowflake};

#[derive(Debug, Clone, FromRow)]
pub struct LoggedInEvent {
    pub event_id: Uuid,
    pub player_uuid: Uuid,
    pub created_at: Timestamp,
    pub username: String,
    #[sqlx(try_from = "crate::extractors::IpAddrString")]
    pub ip_address: IpAddr,
    #[sqlx(rename = "type")]
    pub kind: McAccountType,
    pub member_id: Option<Id<UserMarker>>,
}

/// Error type representing a failure to query with the [`LoggedInEvent`] table.
#[derive(Debug, Error)]
#[error("Failed to query logged in event table from the database")]
pub struct LoggedInEventQueryError;

#[derive(Builder, Debug, Deserialize, Serialize)]
pub struct NewLoggedInEvent {
    #[builder(default = Uuid::new_v4())]
    pub event_id: Uuid,
    pub player_uuid: Uuid,
    #[builder(default = Timestamp::now())]
    pub created_at: Timestamp,
    pub username: Option<String>,
    pub ip_address: IpAddr,
    pub kind: McAccountType,
    pub member_id: Option<Id<UserMarker>>,
}

impl NewLoggedInEvent {
    pub async fn create(
        &self,
        conn: &mut Transaction<'_>,
    ) -> Result<(), Report<LoggedInEventQueryError>> {
        sqlx::query(
            r#"
            INSERT INTO logged_in_events (
                event_id, player_uuid, created_at, username,
                ip_address, "type", member_id
            )
            VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(self.event_id)
        .bind(self.player_uuid)
        .bind(self.created_at)
        .bind(&self.username)
        .bind(self.ip_address.to_string())
        .bind(self.kind)
        .bind(self.member_id.map(|v| Snowflake::new(v.cast())))
        .execute(&mut **conn)
        .await
        .change_context(LoggedInEventQueryError)
        .attach("while trying to log login event")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::member::NewMember;
    use std::str::FromStr;

    #[tokio::test]
    async fn create_should_insert_logged_in_event_with_member() {
        let pool = crate::testing::setup().await;
        let mut tx = pool.begin().await.unwrap();

        let member_id = Id::new(123456789);
        let member = NewMember::builder()
            .discord_user_id(member_id)
            .name("PlayerOne")
            .build();
        member.insert(&mut tx).await.unwrap();

        let player_uuid = Uuid::from_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let event = NewLoggedInEvent::builder()
            .player_uuid(player_uuid)
            .username("PlayerOne".to_string())
            .ip_address(IpAddr::from_str("192.168.1.1").unwrap())
            .kind(McAccountType::Java)
            .member_id(member_id)
            .build();

        event.create(&mut tx).await.unwrap();
        
        // Verify the event was created
        let (db_uuid, db_username, db_ip, db_type): (Uuid, String, String, McAccountType) = 
            sqlx::query_as(
                "SELECT player_uuid, username, ip_address, type FROM logged_in_events WHERE player_uuid = ?"
            )
            .bind(player_uuid)
            .fetch_one(&mut *tx)
            .await
            .unwrap();

        assert_eq!(db_uuid, player_uuid);
        assert_eq!(db_username, "PlayerOne");
        assert_eq!(db_ip, "192.168.1.1");
        assert_eq!(db_type, McAccountType::Java);

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn create_should_insert_logged_in_event_without_member() {
        let pool = crate::testing::setup().await;
        let mut tx = pool.begin().await.unwrap();

        let player_uuid = Uuid::from_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();
        let event = NewLoggedInEvent::builder()
            .player_uuid(player_uuid)
            .username("GuestPlayer".to_string())
            .ip_address(IpAddr::from_str("10.0.0.1").unwrap())
            .kind(McAccountType::Bedrock)
            .build();

        event.create(&mut tx).await.unwrap();
        
        // Verify the event was created
        let (db_uuid, db_username): (Uuid, String) = 
            sqlx::query_as(
                "SELECT player_uuid, username FROM logged_in_events WHERE player_uuid = ?"
            )
            .bind(player_uuid)
            .fetch_one(&mut *tx)
            .await
            .unwrap();

        assert_eq!(db_uuid, player_uuid);
        assert_eq!(db_username, "GuestPlayer");

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn create_should_handle_ipv6_addresses() {
        let pool = crate::testing::setup().await;
        let mut tx = pool.begin().await.unwrap();

        let player_uuid = Uuid::from_str("7c9e6679-7425-40de-944b-e07fc1f90ae7").unwrap();
        let ipv6_addr = IpAddr::from_str("2001:0db8:85a3:0000:0000:8a2e:0370:7334").unwrap();
        
        let event = NewLoggedInEvent::builder()
            .player_uuid(player_uuid)
            .ip_address(ipv6_addr)
            .kind(McAccountType::Java)
            .build();

        event.create(&mut tx).await.unwrap();
        
        // Verify the IPv6 address was stored correctly
        let db_ip: String = sqlx::query_scalar(
            "SELECT ip_address FROM logged_in_events WHERE player_uuid = ?"
        )
        .bind(player_uuid)
        .fetch_one(&mut *tx)
        .await
        .unwrap();

        let stored_addr: IpAddr = db_ip.parse().unwrap();
        assert_eq!(stored_addr, ipv6_addr);

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn create_should_allow_multiple_events_for_same_player() {
        let pool = crate::testing::setup().await;
        let mut tx = pool.begin().await.unwrap();

        let player_uuid = Uuid::from_str("550e8400-e29b-41d4-a716-446655440001").unwrap();
        
        // First login event
        let event1 = NewLoggedInEvent::builder()
            .player_uuid(player_uuid)
            .username("Player".to_string())
            .ip_address(IpAddr::from_str("192.168.1.1").unwrap())
            .kind(McAccountType::Java)
            .build();

        event1.create(&mut tx).await.unwrap();

        // Second login event
        let event2 = NewLoggedInEvent::builder()
            .player_uuid(player_uuid)
            .username("Player".to_string())
            .ip_address(IpAddr::from_str("192.168.1.2").unwrap())
            .kind(McAccountType::Java)
            .build();

        event2.create(&mut tx).await.unwrap();
        
        // Verify both events exist
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM logged_in_events WHERE player_uuid = ?"
        )
        .bind(player_uuid)
        .fetch_one(&mut *tx)
        .await
        .unwrap();

        assert_eq!(count, 2);

        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn create_should_use_custom_event_id_and_timestamp() {
        let pool = crate::testing::setup().await;
        let mut tx = pool.begin().await.unwrap();

        let custom_event_id = Uuid::from_str("12345678-1234-1234-1234-123456789012").unwrap();
        let player_uuid = Uuid::from_str("87654321-4321-4321-4321-210987654321").unwrap();
        let custom_timestamp = Timestamp::from_str("2024-01-01T12:00:00Z").unwrap();
        
        let event = NewLoggedInEvent::builder()
            .event_id(custom_event_id)
            .player_uuid(player_uuid)
            .created_at(custom_timestamp)
            .username("CustomPlayer".to_string())
            .ip_address(IpAddr::from_str("203.0.113.1").unwrap())
            .kind(McAccountType::Bedrock)
            .build();

        event.create(&mut tx).await.unwrap();
        
        // Verify custom values were used
        let (db_event_id, db_created_at): (Uuid, Timestamp) = 
            sqlx::query_as(
                "SELECT event_id, created_at FROM logged_in_events WHERE event_id = ?"
            )
            .bind(custom_event_id)
            .fetch_one(&mut *tx)
            .await
            .unwrap();

        assert_eq!(db_event_id, custom_event_id);
        assert_eq!(db_created_at, custom_timestamp);

        tx.rollback().await.unwrap();
    }
}
