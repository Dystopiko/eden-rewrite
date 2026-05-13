use twilight_model::gateway::payload::incoming::Ready;

use crate::context::EventContext;

pub fn handle(ctx: &EventContext, ready: &Ready) {
    tracing::debug!(
        application.id = %ready.application.id,
        guilds = ready.guilds.len(),
        user.id = %ready.user.id,
        user.name = %ready.user.name,
        "successfully identified"
    );
    ctx.bot_user_id.store(ready.application.id);
}
