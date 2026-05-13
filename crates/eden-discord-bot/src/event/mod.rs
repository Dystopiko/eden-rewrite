use twilight_gateway::Event;

use crate::context::EventContext;

mod ready;

#[allow(clippy::single_match, clippy::match_single_binding)]
#[tracing::instrument(
    skip_all,
    name = "bot.handle_event",
    fields(
        event.kind = ?event.kind(),
        shard.id = %ctx.shard.id(),
        shard.latency = ?ctx.shard.latency(),
    ),
)]
pub async fn handle(ctx: EventContext, event: Event) {
    tracing::trace!("received event");
    match event {
        Event::GatewayHeartbeatAck => {}
        Event::GuildCreate(..) => {}
        Event::MemberUpdate(..) => {}
        Event::MessageCreate(..) => {}
        Event::Ready(ready) => self::ready::handle(&ctx, &ready),
        _ => {}
    };
}
