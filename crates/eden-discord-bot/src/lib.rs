use crossbeam::atomic::AtomicCell;
use eden_common::DatabasePools;
use eden_config::{Config, LiveConfig, types::organization::Discord};
use eden_metrics::MetricsAdapter;
use eden_signals::ShutdownSignal;
use error_stack::{Report, ResultExt};
use futures::StreamExt;
use splinter::{
    ShardConfig, ShardEventStream, ShardHandle, ShardManager,
    config::reconnect_strategies::AlwaysReconnect,
};
use std::sync::Arc;
use thiserror::Error;
use tokio::time::{MissedTickBehavior, timeout};
use tokio_util::task::TaskTracker;
use tracing::{debug, info, trace, warn};
use twilight_gateway::{CloseFrame, queue::InMemoryQueue};
use twilight_model::id::Id;
use twilight_standby::Standby;

use crate::{
    constants::{
        EVENT_TYPE_FLAGS, INTENTS, SHARDING_RANGE, SUPERVISOR_CHECK_INTERVAL, WAIT_TIMEOUT,
    },
    context::{BotContext, EventContext},
};

pub mod constants;
pub mod context;
pub mod infra;

mod event;

#[derive(Debug, Error)]
pub enum BotError {
    #[error("Failed to start Discord bot service")]
    Start,

    #[error("A fatal error occurred in Discord bot service")]
    FatallyClosed,
}

pub fn service(
    config: &Config,
    global_shutdown_signal: ShutdownSignal,
    metrics: Option<Arc<dyn MetricsAdapter>>,
    pools: DatabasePools,
    discord_cfg: &Discord,
) -> BotService {
    let http = twilight_http::Client::builder()
        .token(discord_cfg.token.as_str().to_string())
        .build();

    let bot_live_config = LiveConfig::new(config.clone());
    let ctx = BotContext::builder()
        .bot_live_config(bot_live_config)
        .bot_shutdown_signal(ShutdownSignal::new())
        .bot_user_id(Arc::new(AtomicCell::new(Id::new(1))))
        .global_shutdown_signal(global_shutdown_signal)
        .http(Arc::new(http))
        .maybe_metrics(metrics)
        .pools(pools)
        .build();

    let mut shard_config = ShardConfig::new(
        discord_cfg.token.as_str().to_string(),
        INTENTS,
        InMemoryQueue::default(),
    );

    shard_config.event_type_flags = EVENT_TYPE_FLAGS;
    shard_config.reconnect_strategy = Some(Arc::new(AlwaysReconnect));

    BotService { ctx, shard_config }
}

pub struct BotService {
    pub ctx: Arc<BotContext>,
    shard_config: ShardConfig,
}

impl BotService {
    pub async fn start(self) -> Result<(), Report<BotError>> {
        let ctx = self.ctx;

        let (shard_manager, events) = ShardManager::new(self.shard_config, SHARDING_RANGE);
        let tasks = TaskTracker::new();

        debug!("spawning {} shard(s)", shard_manager.total());
        shard_manager.spawn_all().await;

        let identified = ctx
            .global_shutdown_signal
            .run_result_or_cancelled(wait_until_all_identified(&shard_manager))
            .await?
            .is_some();

        let result = if identified {
            tracing::info!("Discord bot service started successfully");
            tokio::spawn(dispatch_events(ctx.clone(), tasks.clone(), events));
            supervise_shards(&ctx, &shard_manager).await
        } else {
            Ok(())
        };

        // Graceful shutdown: stop accepting new tasks, drain in-flight event
        // handlers, then close every shard connection.
        debug!("closing {} shard(s)...", shard_manager.total());
        tasks.close();

        let remaining = tasks.len();
        if remaining > 0 {
            tracing::warn!("waiting for {remaining} event(s) to be processed");
            tasks.wait().await;
        }

        for shard in shard_manager.shards().await {
            _ = shard.close(CloseFrame::NORMAL);
        }

        shard_manager.shutdown_all().await;
        info!("successfully closed {} shard(s)", shard_manager.total());

        result
    }
}

async fn supervise_shards(
    ctx: &BotContext,
    shard_manager: &ShardManager,
) -> Result<(), Report<BotError>> {
    let mut interval = tokio::time::interval(SUPERVISOR_CHECK_INTERVAL);
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    // Consume the first, immediate tick so the first real check
    // happens after one full interval has elapsed.
    interval.tick().await;

    loop {
        // Block until the next health-check tick, or until shutdown fires.
        // `run_or_cancelled` returns `None` when the shutdown signal arrives.
        let cancelled = ctx
            .global_shutdown_signal
            .run_or_cancelled(interval.tick())
            .await
            .is_none();

        if cancelled {
            break Ok(());
        }

        trace!("performing periodic shard health check");
        let Some(unhealthy) = collect_unhealthy_shards(shard_manager).await else {
            continue;
        };

        // Reconnects disconnected shards concurrently
        let futures: Vec<_> = unhealthy
            .iter()
            .map(|shard| async move { (shard.id(), shard.identified().await) })
            .collect();

        for (id, result) in futures::future::join_all(futures).await {
            if let Err(error) = result {
                warn!(
                    ?error,
                    "shard {id} encountered a fatal error; initiating shutdown"
                );
                ctx.global_shutdown_signal.initiate();
                return Err(Report::new(error).change_context(BotError::FatallyClosed));
            }
            debug!("shard {id} recovered and is healthy again");
        }

        tracing::trace!("{} shard(s) are healthy", unhealthy.len());
    }
}

/// Forwards incoming gateway events to per-event handler tasks.
async fn dispatch_events(ctx: Arc<BotContext>, tasks: TaskTracker, mut stream: ShardEventStream) {
    tracing::debug!("event dispatcher started");

    let standby = Arc::new(Standby::new());
    while let Some((shard, event)) = stream.next().await {
        standby.process(&event);

        let event_ctx = EventContext::builder()
            .bot(ctx.clone())
            .shard(shard)
            .standby(standby.clone())
            .build();

        tasks.spawn(self::event::handle(event_ctx, event));

        if let Some(metrics) = ctx.metrics.as_ref() {}
    }

    tracing::debug!("event dispatcher stopped (stream exhausted)");
}

/// Returns shards that are not healthy, or `None` if every shard is healthy.
async fn collect_unhealthy_shards(shard_manager: &ShardManager) -> Option<Vec<ShardHandle>> {
    let shards = shard_manager.shards().await;
    tracing::trace!("checking health of {} shard(s)", shards.len());

    let disconnected: Vec<_> = shards
        .into_iter()
        .filter(|s| !s.state().is_identified())
        .collect();

    if disconnected.is_empty() {
        trace!("all shard(s) are healthy");
        return None;
    }

    debug!("{} shard(s) got disconnected", disconnected.len());
    Some(disconnected)
}

async fn wait_until_all_identified(shard_manager: &ShardManager) -> Result<(), Report<BotError>> {
    let shards = shard_manager.shards().await;
    let futures: Vec<_> = shards.iter().map(|s| s.identified()).collect();

    let wait = futures::future::try_join_all(futures);
    let Ok(result) = timeout(WAIT_TIMEOUT, wait).await else {
        tracing::warn!("shard identification took longer than expected, skipping wait");
        return Ok(());
    };

    result.change_context(BotError::Start).map(|_| ())
}
