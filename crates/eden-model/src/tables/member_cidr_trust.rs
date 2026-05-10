use bon::Builder;
use eden_timestamp::Timestamp;
use error_stack::{Report, ResultExt};
use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::net::IpAddr;
use thiserror::Error;
use twilight_model::id::{Id, marker::UserMarker};
use uuid::Uuid;

use self::new_member_cidr_trust_builder::SetCidr;
use crate::{common::ApprovalStatus, snowflake::Snowflake};

#[derive(Clone, Debug, Deserialize, FromRow, Serialize)]
pub struct MemberCidrTrust {
    pub id: Uuid,
    pub member_id: Snowflake,
    pub cidr: IpNet,
    pub created_at: Timestamp,
    pub status: ApprovalStatus,
    pub updated_at: Option<Timestamp>,
}

impl MemberCidrTrust {
    pub async fn find(
        conn: &mut eden_postgres::Connection,
        member_id: Id<UserMarker>,
        ip: IpAddr,
    ) -> Result<MemberCidrTrust, Report<MemberCidrTrustQueryError>> {
        sqlx::query_as(
            r#"
            SELECT * FROM member_cidr_trust
            WHERE $1::cidr << cidr
              AND member_id = $2"#,
        )
        .bind(ip)
        .bind(Snowflake::new(member_id.cast()))
        .fetch_one(conn)
        .await
        .change_context(MemberCidrTrustQueryError)
        .attach("while trying to find one")
    }

    pub async fn fetch_all_from_member(
        conn: &mut eden_postgres::Connection,
        id: Id<UserMarker>,
    ) -> Result<Vec<MemberCidrTrust>, Report<MemberCidrTrustQueryError>> {
        sqlx::query_as(
            r#"
            SELECT * FROM member_cidr_trust
            WHERE member_id = $1"#,
        )
        .bind(Snowflake::new(id.cast()))
        .fetch_all(conn)
        .await
        .change_context(MemberCidrTrustQueryError)
        .attach("while trying to fetch all rows by member id")
    }
}

#[derive(Builder)]
pub struct NewMemberCidrTrust {
    #[builder(default = Uuid::new_v4())]
    pub id: Uuid,
    pub member_id: Id<UserMarker>,
    pub cidr: IpNet,
    #[builder(default = ApprovalStatus::Pending)]
    pub status: ApprovalStatus,
    pub updated_at: Option<Timestamp>,
}

impl NewMemberCidrTrust {
    pub async fn insert(
        &self,
        conn: &mut eden_postgres::Transaction<'_>,
    ) -> Result<MemberCidrTrust, Report<MemberCidrTrustQueryError>> {
        sqlx::query_as(
            r#"
            INSERT INTO member_cidr_trust (id, member_id, cidr, status, updated_at)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *"#,
        )
        .bind(self.id)
        .bind(Snowflake::new(self.member_id.cast()))
        .bind(self.cidr)
        .bind(self.status)
        .bind(self.updated_at)
        .fetch_one(&mut **conn)
        .await
        .change_context(MemberCidrTrustQueryError)
        .attach("while trying to insert member_cidr_trust table")
    }
}

impl<S> NewMemberCidrTrustBuilder<S>
where
    S: new_member_cidr_trust_builder::State,
{
    /// Derives a CIDR block from a raw IP address and sets it on the builder.
    ///
    /// This function normalizes an IP address into a CIDR representation suitable for
    /// grouping and access control purposes (e.g. login/IP tracking or abuse detection).
    ///
    /// # Normalization rules
    /// - **IPv4 addresses** are normalized to a `/24` network.
    /// - **IPv6 addresses** are normalized to a `/56` network (complies with [RFC 6177]).
    ///
    /// [RFC 6177]: https://www.rfc-editor.org/rfc/rfc6177
    pub fn cidr_from_ip(self, ip: IpAddr) -> NewMemberCidrTrustBuilder<SetCidr<S>>
    where
        S::Cidr: new_member_cidr_trust_builder::IsUnset,
    {
        let cidr: IpNet = match ip {
            IpAddr::V4(v4) => Ipv4Net::new_assert(v4, 24).into(),

            // XX::/56 is used as outlined in RFC 6177 in Section 1 to 2:
            // https://www.rfc-editor.org/rfc/rfc6177
            IpAddr::V6(v6) => Ipv6Net::new_assert(v6, 56).into(),
        };
        self.cidr(cidr.trunc())
    }
}

/// Error type representing a failure to query with the [`IpAddressControl`] table.
#[derive(Debug, Error)]
#[error("Failed to query member_cidr_trust table from the database")]
pub struct MemberCidrTrustQueryError;

#[cfg(test)]
mod tests {
    use claims::{assert_none, assert_some};
    use eden_postgres::error::QueryResultExt;
    use eden_timestamp::Timestamp;
    use insta::assert_debug_snapshot;
    use ipnet::IpNet;
    use std::{net::IpAddr, str::FromStr};
    use twilight_model::id::Id;
    use uuid::Uuid;

    use crate::{
        tables::member_cidr_trust::{MemberCidrTrust, NewMemberCidrTrust},
        testing::krate::member_with_linked_mc_account,
    };

    #[sqlx::test(migrations = "../../migrations")]
    async fn should_find_member_cidr_trust_by_ipv4(pool: sqlx::PgPool) {
        let _guard = crate::testing::krate::setup();
        let mut conn = pool.begin().await.unwrap();

        let user_id = Id::new(12345);
        member_with_linked_mc_account()
            .name("john")
            .discord_user_id(user_id)
            .mc_username("john1")
            .conn(&mut conn)
            .call()
            .await;

        let original_ip_addr = IpAddr::from_str("44.87.156.234").unwrap();
        let query = NewMemberCidrTrust::builder()
            .member_id(user_id)
            .cidr_from_ip(original_ip_addr)
            .build();

        query.insert(&mut conn).await.unwrap();

        // Test #1: Exact same IP address
        let result = MemberCidrTrust::find(&mut conn, user_id, original_ip_addr)
            .await
            .optional()
            .unwrap();

        assert_some!(&result);

        // Test #2: Different IP address within the subnet
        let diff_ip_addr = IpAddr::from_str("44.87.156.235").unwrap();
        let result = MemberCidrTrust::find(&mut conn, user_id, diff_ip_addr)
            .await
            .optional()
            .unwrap();

        assert_some!(&result);

        // Test #3: Very different IP address
        let diff_ip_addr = IpAddr::from_str("0.0.0.0").unwrap();
        let result = MemberCidrTrust::find(&mut conn, user_id, diff_ip_addr)
            .await
            .optional()
            .unwrap();

        assert_none!(&result);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn should_find_member_cidr_trust_by_ipv6(pool: sqlx::PgPool) {
        let _guard = crate::testing::krate::setup();
        let mut conn = pool.begin().await.unwrap();

        let user_id = Id::new(12345);
        member_with_linked_mc_account()
            .name("john")
            .discord_user_id(user_id)
            .mc_username("john1")
            .conn(&mut conn)
            .call()
            .await;

        let original_ip_addr = IpAddr::from_str("2607:f8b0:4005:080a::200e").unwrap();
        let query = NewMemberCidrTrust::builder()
            .member_id(user_id)
            .cidr_from_ip(original_ip_addr)
            .build();

        query.insert(&mut conn).await.unwrap();

        // Test #1: Exact same IP address
        let result = MemberCidrTrust::find(&mut conn, user_id, original_ip_addr)
            .await
            .optional()
            .unwrap();

        assert_some!(&result);

        // Test #2: Different IP address within the subnet
        let diff_ip_addr = IpAddr::from_str("2607:f8b0:4005:080a::200d").unwrap();
        let result = MemberCidrTrust::find(&mut conn, user_id, diff_ip_addr)
            .await
            .optional()
            .unwrap();

        assert_some!(&result);

        // Test #3: Very different IP address
        let diff_ip_addr = IpAddr::from_str("fe80::").unwrap();
        let result = MemberCidrTrust::find(&mut conn, user_id, diff_ip_addr)
            .await
            .optional()
            .unwrap();

        assert_none!(&result);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn should_insert_member_cidr_trust_with_ipv4(pool: sqlx::PgPool) {
        let _guard = crate::testing::krate::setup();
        let mut conn = pool.begin().await.unwrap();

        let user_id = Id::new(12345);
        member_with_linked_mc_account()
            .name("john")
            .discord_user_id(user_id)
            .mc_username("john1")
            .conn(&mut conn)
            .call()
            .await;

        let query = NewMemberCidrTrust::builder()
            .member_id(user_id)
            .cidr_from_ip(IpAddr::from_str("44.87.156.234").unwrap())
            .build();

        query.insert(&mut conn).await.unwrap();

        let mut result = MemberCidrTrust::fetch_all_from_member(&mut conn, user_id)
            .await
            .unwrap()
            .remove(0);

        result.id = Uuid::nil();
        result.created_at = Timestamp::from_secs(0).unwrap();

        assert_debug_snapshot!(result);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn should_insert_member_cidr_trust_with_ipv6(pool: sqlx::PgPool) {
        let _guard = crate::testing::krate::setup();
        let mut conn = pool.begin().await.unwrap();

        let user_id = Id::new(12345);
        member_with_linked_mc_account()
            .name("john")
            .discord_user_id(user_id)
            .mc_username("john1")
            .conn(&mut conn)
            .call()
            .await;

        let query = NewMemberCidrTrust::builder()
            .member_id(user_id)
            .cidr_from_ip(IpAddr::from_str("2607:f8b0:4005:080a::200e").unwrap())
            .build();

        query.insert(&mut conn).await.unwrap();

        let mut result = MemberCidrTrust::fetch_all_from_member(&mut conn, user_id)
            .await
            .unwrap()
            .remove(0);

        result.id = Uuid::nil();
        result.created_at = Timestamp::from_secs(0).unwrap();

        assert_debug_snapshot!(result);
    }

    #[test]
    fn should_normalize_ip_into_ipnet() {
        let cidr = NewMemberCidrTrust::builder()
            .cidr_from_ip(IpAddr::from_str("127.1.2.3").unwrap())
            .member_id(Id::new(123))
            .build()
            .cidr;

        assert_eq!(cidr, IpNet::from_str("127.1.2.0/24").unwrap());

        let cidr = NewMemberCidrTrust::builder()
            .cidr_from_ip(IpAddr::from_str("2001:db8::1234:5678").unwrap())
            .member_id(Id::new(123))
            .build()
            .cidr;

        assert_eq!(cidr, IpNet::from_str("2001:db8::/56").unwrap());
    }
}
