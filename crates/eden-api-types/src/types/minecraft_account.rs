use eden_minecraft_types::McEdition;
use eden_timestamp::Timestamp;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use uuid::fmt::Hyphenated;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FullMcAccount {
    pub uuid: Hyphenated,
    pub username: String,
    pub edition: McEdition,
    pub linked_at: Timestamp,
    pub last_login_at: Option<Timestamp>,
    pub last_ip_address: Option<IpAddr>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MinimalMcAccount {
    pub uuid: Hyphenated,
    pub username: String,
    pub edition: McEdition,
    pub linked_at: Timestamp,
}
