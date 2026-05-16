use eden_api_server::WebContext;
use eden_background_worker::runner::Runner;
use eden_common::{AppContext, token::RawToken};
use eden_jobs::{JobContext, RunnerExt};
use eden_model::tables::{
    member::NewMember,
    staff::NewStaff,
    tokens::{NewToken, PermissionScope, TokenType},
};
use eden_signals::ShutdownSignal;
use std::sync::Arc;
use twilight_model::id::{Id, marker::UserMarker};

mod builder;
mod mock_users;

use self::builder::MockEdenSystemBuilder;
pub use self::mock_users::*;

pub struct MockEdenSystem {
    runner: Option<Runner<Arc<JobContext>>>,
    shutdown_signal: ShutdownSignal,
    web_context: Arc<WebContext>,
}

impl MockEdenSystem {
    pub fn builder(pool: sqlx::PgPool) -> MockEdenSystemBuilder {
        MockEdenSystemBuilder::new(pool)
    }

    #[must_use]
    pub fn app_context(&self) -> Arc<AppContext> {
        self.web_context.app.clone()
    }

    pub async fn as_unauthenticated_user(&self) -> MockUser {
        MockUser::unauthenticated(&self.web_context.app)
    }

    pub async fn as_admin_user(
        &self,
        id: Id<UserMarker>,
        permissions: PermissionScope,
    ) -> MockUser {
        let mut conn = self.db_conn().await;
        let token = RawToken::generate(TokenType::McServer);
        NewToken::builder()
            .authorized_by("integration-testing")
            .hashed(token.hash().encode())
            .member_id(id)
            .name("mock-mc-server-token")
            .permissions(permissions)
            .build()
            .insert(&mut conn)
            .await
            .unwrap();

        MockUser::with_token(&self.web_context.app, token)
    }

    pub async fn as_mc_server_user(&self) -> MockUser {
        let mut conn = self.db_conn().await;
        let token = RawToken::generate(TokenType::McServer);
        NewToken::builder()
            .authorized_by("integration-testing")
            .hashed(token.hash().encode())
            .name("mock-mc-server-token")
            .build()
            .insert(&mut conn)
            .await
            .unwrap();

        MockUser::with_token(&self.web_context.app, token)
    }

    pub async fn db_conn(&self) -> eden_postgres::PooledConnection {
        self.pools()
            .primary_db()
            .acquire()
            .await
            .expect("could not acquire connection")
    }

    pub async fn db_new_member(&self, discord_user_id: Id<UserMarker>, name: &str) {
        let mut conn = self.db_tx().await;
        NewMember::builder()
            .discord_user_id(discord_user_id)
            .name(name)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        conn.commit().await.unwrap();
    }

    pub async fn db_new_staff(&self, member_id: Id<UserMarker>, admin: bool) {
        let mut conn = self.db_conn().await;
        NewStaff::builder()
            .admin(admin)
            .member_id(member_id)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();
    }

    pub async fn db_tx(&self) -> eden_postgres::Transaction<'_> {
        self.pools()
            .write()
            .await
            .expect("could not acquire transaction")
    }

    pub async fn run_migrations(&self) {
        eden_model::tables::migrations::perform(self.pools().primary_db())
            .await
            .unwrap();
    }

    #[must_use]
    pub fn web_context(&self) -> &Arc<WebContext> {
        &self.web_context
    }
}

impl std::ops::Deref for MockEdenSystem {
    type Target = AppContext;

    fn deref(&self) -> &Self::Target {
        &self.web_context
    }
}

impl MockEdenSystemBuilder {
    pub fn build(self) -> MockEdenSystem {
        let build_job_runner = self.build_job_runner;
        let shutdown_signal = ShutdownSignal::new();
        let app = self.build_app_context(shutdown_signal.clone());

        MockEdenSystem {
            runner: build_job_runner.then(|| {
                let job = JobContext::builder().app(app.clone()).build();
                let pool = job.pools().primary_db().clone();
                let runner = Runner::new(job, pool)
                    .register_eden_job_types()
                    .shutdown_when_queue_empty();

                runner
            }),
            shutdown_signal,
            web_context: Arc::new(WebContext { app: app.clone() }),
        }
    }
}
