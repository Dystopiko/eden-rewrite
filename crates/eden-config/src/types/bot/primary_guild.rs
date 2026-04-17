//! Primary guild configuration.

use crate::validation::Validate;
use serde::Deserialize;

/// Configuration for the primary Discord guild (server).
///
/// The primary guild is the main server where the bot operates and
/// manages its core functionality.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Validate)]
pub struct PrimaryGuild {}
