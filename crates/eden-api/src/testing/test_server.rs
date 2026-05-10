use axum_test::TestServer;
use eden_api_types::eden_minecraft_types::McEdition;
use eden_background_worker::runner::Runner;
use eden_config::{Config, LiveConfig, types::setup::InitialSettings};
use eden_jobs::{JobContext, RunnerExt};
use eden_metrics::MetricsAdapter;
use eden_model::{
    common::ApprovalStatus,
    tables::{
        contributor::NewContributor, linked_mc_account::LinkMcAccount, member::NewMember,
        member_cidr_trust::NewMemberCidrTrust, settings::NewSettings, staff::NewStaff,
    },
};
use eden_postgres::Pool;
use eden_services::{Cache, DatabasePools, DiscordService, cache::NopMemoryCache};
use eden_signals::ShutdownSignal;
use error_stack::Report;
use std::{net::IpAddr, path::Path, sync::Arc};
use thiserror::Error;
use twilight_model::id::{Id, marker::UserMarker};
use uuid::Uuid;

use crate::context::WebContext;

pub struct TestApp(Arc<TestAppInner>);

struct TestAppInner {
    context: Arc<WebContext>,
    runner: Option<Runner<Arc<JobContext>>>,
    shutdown_signal: ShutdownSignal,
}

#[derive(Debug, Error)]
#[error("{0} jobs failed")]
pub struct FailedJobError(i64);

impl TestApp {
    pub fn builder(pool: sqlx::PgPool) -> TestAppBuilder {
        TestAppBuilder::new(pool)
    }

    pub async fn db_set_settings(&self, settings: InitialSettings) {
        let org_guild_id = self
            .config
            .get()
            .organization
            .discord
            .as_ref()
            .expect("[organization.discord] must be configured to set something in settings")
            .guild_id;

        let mut conn = self
            .pools
            .primary_db()
            .begin()
            .await
            .expect("could not acquire connection");

        NewSettings::builder()
            .org_guild_id(org_guild_id)
            .use_initial_settings(&settings)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        conn.commit().await.unwrap();
    }

    pub async fn db_run_migrations(&self) {
        eden_model::tables::migrations::perform(self.pools.primary_db())
            .await
            .unwrap();
    }

    pub async fn assert_no_pending_jobs(&self) {
        let mut conn = self
            .pools
            .primary_db()
            .acquire()
            .await
            .expect("could not acquire connection");

        let pending_jobs = sqlx::query_scalar::<_, Option<serde_json::Value>>(
            "SELECT json_agg(background_jobs) FROM background_jobs",
        )
        .fetch_all(&mut *conn)
        .await
        .unwrap();

        assert_eq!(
            pending_jobs.iter().filter(|v| v.is_some()).count(),
            0,
            "there should be no pending jobs to do: {pending_jobs:#?}"
        );
    }

    pub async fn db_new_member(&self, discord_user_id: Id<UserMarker>, name: &str) {
        let mut conn = self
            .pools
            .write()
            .await
            .expect("could not acquire transaction");

        NewMember::builder()
            .discord_user_id(discord_user_id)
            .name(name)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        conn.commit().await.unwrap();
    }

    pub async fn db_new_contributor(&self, discord_user_id: Id<UserMarker>, name: &str) {
        let mut conn = self
            .pools
            .write()
            .await
            .expect("could not acquire transaction");

        NewMember::builder()
            .discord_user_id(discord_user_id)
            .name(name)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        NewContributor::builder()
            .member_id(discord_user_id)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        conn.commit().await.unwrap();
    }

    pub async fn db_new_staff(&self, discord_user_id: Id<UserMarker>, name: &str, admin: bool) {
        let mut conn = self
            .pools
            .write()
            .await
            .expect("could not acquire transaction");

        NewMember::builder()
            .discord_user_id(discord_user_id)
            .name(name)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        NewStaff::builder()
            .member_id(discord_user_id)
            .admin(admin)
            .build()
            .upsert(&mut conn)
            .await
            .unwrap();

        conn.commit().await.unwrap();
    }

    pub async fn db_link_mc_account(
        &self,
        member_id: Id<UserMarker>,
        uuid: Uuid,
        username: &str,
        edition: McEdition,
    ) {
        let mut conn = self
            .pools
            .write()
            .await
            .expect("could not acquire transaction");

        LinkMcAccount::builder()
            .member_id(member_id)
            .uuid(uuid)
            .username(username)
            .edition(edition)
            .build()
            .insert(&mut conn)
            .await
            .unwrap();

        conn.commit().await.unwrap();
    }

    pub async fn db_revoke_ip(&self, member_id: Id<UserMarker>, ip: IpAddr) {
        let mut conn = self
            .pools
            .write()
            .await
            .expect("could not acquire transaction");

        NewMemberCidrTrust::builder()
            .cidr_from_ip(ip)
            .member_id(member_id)
            .status(ApprovalStatus::Revoked)
            .build()
            .insert(&mut conn)
            .await
            .unwrap();

        conn.commit().await.unwrap();
    }

    pub async fn db_trust_ip(&self, member_id: Id<UserMarker>, ip: IpAddr) {
        let mut conn = self
            .pools
            .write()
            .await
            .expect("could not acquire transaction");

        NewMemberCidrTrust::builder()
            .cidr_from_ip(ip)
            .member_id(member_id)
            .status(ApprovalStatus::Approved)
            .build()
            .insert(&mut conn)
            .await
            .unwrap();

        conn.commit().await.unwrap();
    }

    pub async fn run_pending_background_jobs(&self) -> Result<(), Report<FailedJobError>> {
        let runner = self
            .0
            .runner
            .as_ref()
            .expect("runner has not been initialized");

        runner.start().shutdown().await;
        self.assert_no_failed_jobs().await
    }

    pub fn shutdown(&self) {
        self.0.shutdown_signal.initiate();
    }
}

impl TestApp {
    async fn assert_no_failed_jobs(&self) -> Result<(), Report<FailedJobError>> {
        let mut conn = self.pools.primary_db().acquire().await.unwrap();
        let failed_jobs: i64 =
            sqlx::query_scalar("SELECT count(*) FROM background_jobs WHERE status = 'failed'")
                .fetch_one(&mut *conn)
                .await
                .unwrap();

        if failed_jobs > 0 {
            return Err(Report::new(FailedJobError(failed_jobs)));
        }

        Ok(())
    }
}

impl std::ops::Deref for TestApp {
    type Target = WebContext;

    fn deref(&self) -> &Self::Target {
        &self.0.context
    }
}

#[must_use = "builders do not do anything unless you build them"]
pub struct TestAppBuilder {
    build_job_runner: bool,
    cache: Arc<dyn Cache>,
    config: Option<Config>,
    discord_service: Option<Arc<dyn DiscordService>>,
    metrics: Option<Arc<dyn MetricsAdapter>>,
    pool: Pool,
}

impl TestAppBuilder {
    fn new(pool: sqlx::PgPool) -> Self {
        Self {
            build_job_runner: false,
            cache: Arc::new(NopMemoryCache),
            config: None,
            discord_service: None,
            metrics: None,
            pool: pool.into(),
        }
    }

    pub fn with_cache(mut self, cache: impl Cache) -> Self {
        self.cache = Arc::new(cache);
        self
    }

    pub fn with_config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    pub fn with_discord_service(mut self, service: impl DiscordService + 'static) -> Self {
        self.discord_service = Some(Arc::new(service));
        self
    }

    pub fn with_metrics(mut self, metrics: impl MetricsAdapter + 'static) -> Self {
        self.metrics = Some(Arc::new(metrics));
        self
    }

    pub fn with_runner(mut self) -> Self {
        self.build_job_runner = true;
        self
    }

    pub fn build(self) -> (TestApp, TestServer) {
        let shutdown_signal = ShutdownSignal::new();
        let web_context = self.build_web_context(&shutdown_signal);
        let runner = self.build_runner(&web_context, &shutdown_signal);

        let server = self.build_axum_server(&web_context);
        let app = TestApp(Arc::new(TestAppInner {
            context: web_context,
            runner,
            shutdown_signal,
        }));

        (app, server)
    }

    fn build_axum_server(&self, ctx: &Arc<WebContext>) -> TestServer {
        let app = crate::router::build(ctx.clone());
        TestServer::new(app)
    }

    fn build_web_context(&self, shutdown_signal: &ShutdownSignal) -> Arc<WebContext> {
        let pools = DatabasePools::builder()
            .primary_db(self.pool.clone())
            .maybe_metrics(self.metrics.clone())
            .build();

        WebContext::builder()
            .cache(self.cache.clone())
            .config(LiveConfig::new(self.resolve_config()))
            .maybe_metrics(self.metrics.clone())
            .pools(pools)
            .shutdown_signal(shutdown_signal.clone())
            .build()
    }

    fn build_runner(
        &self,
        web_context: &Arc<WebContext>,
        shutdown_signal: &ShutdownSignal,
    ) -> Option<Runner<Arc<JobContext>>> {
        if !self.build_job_runner {
            return None;
        }

        let discord_service = self
            .discord_service
            .clone()
            .expect("DiscordService is missing");

        let job_context = JobContext::builder()
            .cache(web_context.cache.clone())
            .config(web_context.config.clone())
            .discord(discord_service)
            .maybe_metrics(web_context.metrics.clone())
            .pools(web_context.pools.clone())
            .shutdown_signal(shutdown_signal.clone())
            .build();

        let pool = web_context.pools.primary_db().clone();
        let runner = Runner::new(job_context, pool)
            .register_eden_job_types()
            .shutdown_when_queue_empty();

        Some(runner)
    }

    fn resolve_config(&self) -> Config {
        self.config.clone().unwrap_or_else(|| {
            Config::maybe_toml_file(
                r#"
                [organization.discord]
                guild_id = "123456789"            
                token = "foo"
                "#,
                Path::new(""),
            )
            .expect("failed to parse default config")
            .0
        })
    }
}
