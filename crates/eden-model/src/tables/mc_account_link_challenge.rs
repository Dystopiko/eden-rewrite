use bon::Builder;
use eden_minecraft_types::McEdition;
use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use sqlx::{FromRow, Type};
use std::{net::IpAddr, time::Duration};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, FromRow, PartialEq)]
pub struct McAccountLinkChallenge {
    pub id: Uuid,
    pub created_at: Timestamp,

    pub hashed_code: String,
    pub expires_at: Timestamp,

    pub player_uuid: Uuid,
    pub username: String,
    pub edition: McEdition,

    pub ip_address: IpAddr,
    pub status: ChallengeStatus,
    pub updated_at: Option<Timestamp>,
}

impl McAccountLinkChallenge {
    pub async fn find(
        conn: &mut eden_postgres::Connection,
        id: Uuid,
    ) -> Result<McAccountLinkChallenge, Report<QueryError>> {
        sqlx::query_as("SELECT * FROM mc_account_link_challenges WHERE id = $1")
            .bind(id)
            .fetch_one(conn)
            .await
            .change_context(QueryError)
            .attach("while trying to find a link challenge by a hashed code")
    }

    pub async fn find_by_hashed_code(
        conn: &mut eden_postgres::Connection,
        hashed_code: &str,
    ) -> Result<McAccountLinkChallenge, Report<QueryError>> {
        sqlx::query_as(
            r#"
            SELECT * FROM mc_account_link_challenges
            WHERE hashed_code = $1
              AND status = $2
              AND now() < expires_at"#,
        )
        .bind(hashed_code)
        .bind(ChallengeStatus::InProgress)
        .fetch_one(conn)
        .await
        .change_context(QueryError)
        .attach("while trying to find a link challenge by a hashed code")
    }

    pub async fn find_in_progress(
        conn: &mut eden_postgres::Connection,
        uuid: Uuid,
    ) -> Result<McAccountLinkChallenge, Report<QueryError>> {
        sqlx::query_as(
            r#"
            SELECT * FROM mc_account_link_challenges
            WHERE player_uuid = $1
              AND status = $2
              AND now() < expires_at"#,
        )
        .bind(uuid)
        .bind(ChallengeStatus::InProgress)
        .fetch_one(conn)
        .await
        .change_context(QueryError)
        .attach("while trying to find a link challenge")
    }

    pub async fn mark_status(
        &self,
        conn: &mut eden_postgres::Transaction<'_>,
        status: ChallengeStatus,
    ) -> Result<bool, Report<QueryError>> {
        if status == ChallengeStatus::InProgress {
            return Err(Report::new(QueryError)).attach("tried to mark status as in_progress");
        }

        let result = sqlx::query(
            r#"
            UPDATE mc_account_link_challenges
            SET hashed_code = '<deleted>',
                status = $1,
                updated_at = now()
            WHERE id = $2
              AND status = 'in_progress'"#,
        )
        .bind(status)
        .bind(self.id)
        .execute(&mut **conn)
        .await
        .change_context(QueryError)
        .attach("while trying to mark status to a link challenge")?;

        Ok(result.rows_affected() == 1)
    }
}

#[derive(Builder)]
pub struct NewMcLinkChallenge<'a> {
    pub hashed_code: &'a str,

    #[builder(default = Timestamp::now())]
    pub created_at: Timestamp,
    pub ttl: Duration,

    pub player_uuid: Uuid,
    pub username: &'a str,

    pub edition: McEdition,
    pub ip_address: IpAddr,
}

impl NewMcLinkChallenge<'_> {
    pub async fn insert(
        &self,
        conn: &mut eden_postgres::Transaction<'_>,
    ) -> Result<McAccountLinkChallenge, Report<QueryError>> {
        let ttl = self.ttl.as_secs().try_into().ok();
        let expires_at = ttl
            .and_then(|ttl: i64| self.created_at.timestamp().checked_add(ttl))
            .and_then(|v| Timestamp::from_secs(v).ok())
            .unwrap_or(self.created_at);

        // Cancel any existing in-progress attempts for same uuid or username.
        sqlx::query(
            r#"
            UPDATE mc_account_link_challenges
            SET hashed_code = '<deleted>',
                status = $1,
                updated_at = now()
            WHERE status = $2
              AND (player_uuid = $3 OR username = $4)"#,
        )
        .bind(ChallengeStatus::Cancelled)
        .bind(ChallengeStatus::InProgress)
        .bind(self.player_uuid)
        .bind(self.username)
        .execute(&mut **conn)
        .await
        .change_context(QueryError)
        .attach("while trying to cancel existing challenge")?;

        sqlx::query_as(
            r#"
            INSERT INTO mc_account_link_challenges (
                id, hashed_code, expires_at, player_uuid,
                username, edition, ip_address
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *"#,
        )
        .bind(Uuid::new_v4())
        .bind(self.hashed_code)
        .bind(expires_at)
        .bind(self.player_uuid)
        .bind(self.username)
        .bind(self.edition)
        .bind(self.ip_address)
        .fetch_one(&mut **conn)
        .await
        .change_context(QueryError)
        .attach("while trying to insert a new challenge")
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash, Type)]
#[sqlx(type_name = "mc_challenge_status", rename_all = "snake_case")]
pub enum ChallengeStatus {
    #[default]
    InProgress,
    Cancelled,
    Done,
}

#[derive(Debug, Error)]
#[error("could not query mc_account_link_challenge table")]
pub struct QueryError;

#[cfg(test)]
mod tests {
    use claims::{assert_none, assert_ok, assert_some};
    use eden_minecraft_types::McEdition;
    use eden_postgres::error::QueryResultExt;
    use eden_timestamp::Timestamp;
    use insta::assert_debug_snapshot;
    use std::{
        net::{IpAddr, Ipv4Addr},
        str::FromStr,
        time::Duration,
    };
    use uuid::Uuid;

    use crate::tables::mc_account_link_challenge::{
        ChallengeStatus, McAccountLinkChallenge, NewMcLinkChallenge,
    };

    #[sqlx::test(migrations = "../../migrations")]
    async fn should_only_find_by_hashed_if_its_in_progress(pool: sqlx::PgPool) {
        let _guard = crate::testing::krate::setup();

        let mut conn = pool.begin().await.unwrap();
        let challenge = NewMcLinkChallenge::builder()
            .hashed_code("hello")
            .ttl(Duration::from_secs(30))
            .player_uuid(Uuid::from_str("09b52801-111f-44ba-81a6-3c8130b6122c").unwrap())
            .edition(McEdition::Java)
            .ip_address(IpAddr::V4(Ipv4Addr::LOCALHOST))
            .username("steve")
            .build()
            .insert(&mut conn)
            .await
            .unwrap();

        let result = McAccountLinkChallenge::find_by_hashed_code(&mut conn, "hello")
            .await
            .optional();

        assert_ok!(&result);

        let query = result.unwrap();
        assert_some!(&query);

        challenge
            .mark_status(&mut conn, ChallengeStatus::Done)
            .await
            .unwrap();

        let result = McAccountLinkChallenge::find_by_hashed_code(&mut conn, "hello")
            .await
            .optional();

        assert_ok!(&result);

        let challenge = result.unwrap();
        assert_none!(&challenge);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn should_only_find_in_progress_if_its_in_progress(pool: sqlx::PgPool) {
        let _guard = crate::testing::krate::setup();

        let mut conn = pool.begin().await.unwrap();
        let challenge = NewMcLinkChallenge::builder()
            .hashed_code("hello")
            .ttl(Duration::from_secs(30))
            .player_uuid(Uuid::from_str("09b52801-111f-44ba-81a6-3c8130b6122c").unwrap())
            .edition(McEdition::Java)
            .ip_address(IpAddr::V4(Ipv4Addr::LOCALHOST))
            .username("steve")
            .build()
            .insert(&mut conn)
            .await
            .unwrap();

        let result = McAccountLinkChallenge::find_in_progress(&mut conn, challenge.player_uuid)
            .await
            .optional();

        assert_ok!(&result);

        let query = result.unwrap();
        assert_some!(&query);

        challenge
            .mark_status(&mut conn, ChallengeStatus::Done)
            .await
            .unwrap();

        let result = McAccountLinkChallenge::find_in_progress(&mut conn, challenge.id)
            .await
            .optional();

        assert_ok!(&result);

        let challenge = result.unwrap();
        assert_none!(&challenge);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn should_not_set_status_if_already_marked(pool: sqlx::PgPool) {
        let _guard = crate::testing::krate::setup();

        let mut conn = pool.begin().await.unwrap();
        let challenge = NewMcLinkChallenge::builder()
            .hashed_code("hello")
            .created_at(Timestamp::from_secs(0).unwrap())
            .ttl(Duration::from_secs(30))
            .player_uuid(Uuid::from_str("09b52801-111f-44ba-81a6-3c8130b6122c").unwrap())
            .edition(McEdition::Java)
            .ip_address(IpAddr::V4(Ipv4Addr::LOCALHOST))
            .username("steve")
            .build()
            .insert(&mut conn)
            .await
            .unwrap();

        challenge
            .mark_status(&mut conn, ChallengeStatus::Done)
            .await
            .unwrap();

        let result = challenge
            .mark_status(&mut conn, ChallengeStatus::Done)
            .await;

        assert_ok!(&result);
        assert!(!result.unwrap());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn should_mark_mc_account_link_challenge_in_any_status(pool: sqlx::PgPool) {
        let _guard = crate::testing::krate::setup();

        let mut conn = pool.begin().await.unwrap();
        let challenge = NewMcLinkChallenge::builder()
            .hashed_code("hello")
            .created_at(Timestamp::from_secs(0).unwrap())
            .ttl(Duration::from_secs(30))
            .player_uuid(Uuid::from_str("09b52801-111f-44ba-81a6-3c8130b6122c").unwrap())
            .edition(McEdition::Java)
            .ip_address(IpAddr::V4(Ipv4Addr::LOCALHOST))
            .username("steve")
            .build()
            .insert(&mut conn)
            .await
            .unwrap();

        let result = challenge
            .mark_status(&mut conn, ChallengeStatus::Done)
            .await;

        assert_ok!(&result);
        assert!(result.unwrap());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn should_cancel_in_progress_challenges_when_inserting(pool: sqlx::PgPool) {
        let _guard = crate::testing::krate::setup();

        let player_uuid = Uuid::from_str("09b52801-111f-44ba-81a6-3c8130b6122c").unwrap();
        let username = "steve";

        let mut conn = pool.begin().await.unwrap();
        let challenge_1 = NewMcLinkChallenge::builder()
            .hashed_code("hello")
            .created_at(Timestamp::from_secs(0).unwrap())
            .ttl(Duration::from_secs(30))
            .player_uuid(player_uuid)
            .edition(McEdition::Java)
            .ip_address(IpAddr::V4(Ipv4Addr::LOCALHOST))
            .username(username)
            .build()
            .insert(&mut conn)
            .await
            .unwrap()
            .id;

        let result = NewMcLinkChallenge::builder()
            .hashed_code("world")
            .created_at(Timestamp::from_secs(1).unwrap())
            .ttl(Duration::from_secs(121))
            .player_uuid(player_uuid)
            .edition(McEdition::Java)
            .ip_address(IpAddr::V4(Ipv4Addr::LOCALHOST))
            .username(username)
            .build()
            .insert(&mut conn)
            .await;

        assert_ok!(&result);

        let mut challenge = McAccountLinkChallenge::find(&mut conn, challenge_1)
            .await
            .unwrap();

        assert_eq!(challenge.status, ChallengeStatus::Cancelled);
        challenge.id = Uuid::nil();
        challenge.created_at = Timestamp::from_secs(0).unwrap();

        assert!(challenge.updated_at.is_some());
        challenge.updated_at = Some(Timestamp::from_secs(10).unwrap());

        assert_debug_snapshot!(challenge);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn should_insert_mc_account_link_challenge(pool: sqlx::PgPool) {
        let _guard = crate::testing::krate::setup();

        let mut conn = pool.begin().await.unwrap();
        let query = NewMcLinkChallenge::builder()
            .hashed_code("hello")
            .created_at(Timestamp::from_secs(0).unwrap())
            .ttl(Duration::from_secs(30))
            .player_uuid(Uuid::from_str("09b52801-111f-44ba-81a6-3c8130b6122c").unwrap())
            .edition(McEdition::Java)
            .ip_address(IpAddr::V4(Ipv4Addr::LOCALHOST))
            .username("steve")
            .build();

        let result = query.insert(&mut conn).await;
        assert_ok!(&result);

        let mut challenge = result.unwrap();
        challenge.id = Uuid::nil();
        challenge.created_at = Timestamp::from_secs(0).unwrap();

        assert_debug_snapshot!(challenge);
    }
}
