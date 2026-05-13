use bon::Builder;
use crossbeam::atomic::AtomicCell;
use eden_common::DatabasePools;
use eden_config::LiveConfig;
use eden_metrics::MetricsAdapter;
use eden_signals::ShutdownSignal;
use splinter::ShardHandle;
use std::{ops::Deref, sync::Arc};
use twilight_model::id::{Id, marker::ApplicationMarker};
use twilight_standby::Standby;

#[derive(Builder, Debug)]
pub struct EventContext {
    pub shard: ShardHandle,
    pub standby: Arc<Standby>,
    bot: Arc<BotContext>,
}

impl Deref for EventContext {
    type Target = BotContext;

    fn deref(&self) -> &Self::Target {
        &self.bot
    }
}

#[derive(Builder, Debug)]
#[builder(finish_fn(vis = "", name = "build_inner"))]
pub struct BotContext {
    pub bot_live_config: LiveConfig,
    pub bot_shutdown_signal: ShutdownSignal,
    pub bot_user_id: Arc<AtomicCell<Id<ApplicationMarker>>>,
    pub global_shutdown_signal: ShutdownSignal,
    pub http: Arc<twilight_http::Client>,
    pub metrics: Option<Arc<dyn MetricsAdapter>>,
    pub pools: DatabasePools,
}

impl<S> BotContextBuilder<S>
where
    S: bot_context_builder::State,
{
    #[must_use]
    pub fn build(self) -> Arc<BotContext>
    where
        S: bot_context_builder::IsComplete,
    {
        Arc::new(self.build_inner())
    }
}
