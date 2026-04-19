use doku::Document;
use eden_config_derive::Validate;
use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr};

pub mod tls;
pub mod validators;

pub use self::tls::Tls;

#[derive(Clone, Debug, Deserialize, Document, Eq, PartialEq, Validate)]
#[serde(default)]
pub struct Gateway {
    #[validate(skip)]
    pub ip: IpAddr,
    #[validate(skip)]
    pub port: u16,
    pub tls: Option<Tls>,
}

impl Gateway {
    pub const DEFAULT_IP: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

    // Inspired from a popular nature park in the Philippines
    pub const DEFAULT_PORT: u16 = 7590;
}

impl Default for Gateway {
    fn default() -> Self {
        Self {
            ip: Self::DEFAULT_IP,
            port: Self::DEFAULT_PORT,
            tls: None,
        }
    }
}
