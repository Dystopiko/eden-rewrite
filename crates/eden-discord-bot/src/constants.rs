use splinter::ShardingRange;
use std::time::Duration;
use twilight_gateway::{EventTypeFlags, Intents};

pub const EVENT_TYPE_FLAGS: EventTypeFlags = EventTypeFlags::READY
    .union(EventTypeFlags::GATEWAY_HEARTBEAT_ACK)
    .union(EventTypeFlags::GUILD_CREATE)
    .union(EventTypeFlags::MEMBER_UPDATE)
    .union(EventTypeFlags::MESSAGE_CREATE);

pub const INTENTS: Intents = Intents::DIRECT_MESSAGES
    .union(Intents::GUILDS)
    .union(Intents::GUILD_MEMBERS)
    .union(Intents::GUILD_MESSAGES)
    .union(Intents::MESSAGE_CONTENT);

pub const SUPERVISOR_CHECK_INTERVAL: Duration = Duration::from_secs(30);
pub const SHARDING_RANGE: ShardingRange = ShardingRange::ONE;

// Minimum wait timeout for all shards to be identified.
//
// If it takes more than the specified duration to be identified,
// it will assume all shards are identified.
pub const WAIT_TIMEOUT: Duration = Duration::from_secs(30);
