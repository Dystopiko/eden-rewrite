use std::net::IpAddr;

use eden_common::token::RawToken;
use eden_model::{
    common::ApprovalStatus,
    tables::{
        member::NewMember,
        member_cidr_trust::NewMemberCidrTrust,
        staff::NewStaff,
        tokens::{NewToken, PermissionScope, TokenType},
    },
};
use twilight_model::id::{Id, marker::UserMarker};

use super::{MockUser, TestHarness};

impl TestHarness {
    /// Creates a [`MockUser`] authenticated as an organization member.
    pub async fn as_member_user(
        &self,
        member_id: Id<UserMarker>,
        permissions: PermissionScope,
    ) -> MockUser {
        let token = RawToken::generate(TokenType::User);
        insert_user_token(member_id, permissions, &token, self).await;
        MockUser::new(self, Some(token))
    }

    /// Creates a [`MockUser`] authenticated as a Minecraft server.
    pub async fn as_minecraft_server(&self) -> MockUser {
        let token = RawToken::generate(TokenType::McServer);
        insert_mc_server_token(&token, self).await;
        MockUser::new(self, Some(token))
    }

    pub async fn db_new_admin_staff(&self, member_id: Id<UserMarker>) {
        let mut tx = self.db_tx().await;
        NewStaff::builder()
            .member_id(member_id)
            .admin(true)
            .build()
            .upsert(&mut tx)
            .await
            .expect("failed to upsert admin");

        tx.commit().await.expect("failed to commit transaction");
    }

    pub async fn db_new_cidr_trust(
        &self,
        member_id: Id<UserMarker>,
        ip: IpAddr,
        status: ApprovalStatus,
    ) {
        let mut tx = self.db_tx().await;
        NewMemberCidrTrust::builder()
            .cidr_from_ip(ip)
            .member_id(member_id)
            .status(status)
            .build()
            .insert(&mut tx)
            .await
            .expect("failed to insert member cidr trust");

        tx.commit().await.expect("failed to commit transaction");
    }

    pub async fn db_new_member(&self, discord_user_id: Id<UserMarker>, name: &str) {
        let mut tx = self.db_tx().await;
        NewMember::builder()
            .discord_user_id(discord_user_id)
            .name(name)
            .build()
            .upsert(&mut tx)
            .await
            .expect("failed to upsert member");

        tx.commit().await.expect("failed to commit transaction");
    }

    pub async fn db_new_staff(&self, member_id: Id<UserMarker>) {
        let mut tx = self.db_tx().await;
        NewStaff::builder()
            .member_id(member_id)
            .admin(false)
            .build()
            .upsert(&mut tx)
            .await
            .expect("failed to upsert staff");

        tx.commit().await.expect("failed to commit transaction");
    }
}

async fn insert_user_token(
    member_id: Id<UserMarker>,
    permissions: PermissionScope,
    token: &RawToken,
    harness: &TestHarness,
) {
    let mut conn = harness.db_conn().await;
    NewToken::builder()
        .authorized_by("integration-testing")
        .hashed(token.hash().encode())
        .name("mock-user-server-token")
        .member_id(member_id)
        .permissions(permissions)
        .build()
        .insert(&mut conn)
        .await
        .expect("failed to insert mock mc-server token");
}

async fn insert_mc_server_token(token: &RawToken, harness: &TestHarness) {
    let mut conn = harness.db_conn().await;
    NewToken::builder()
        .authorized_by("integration-testing")
        .hashed(token.hash().encode())
        .name("mock-mc-server-token")
        .build()
        .insert(&mut conn)
        .await
        .expect("failed to insert mock mc-server token");
}
