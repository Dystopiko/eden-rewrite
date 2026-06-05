use async_trait::async_trait;
use eden_model::{alerts::command::CommandAlert, tables::mc_login_event::McLoginEvent};
use erased_report::ErasedReport;
use futures::future::try_join_all;
use std::sync::Arc;

use crate::domain::{Notifier, notifier::LinkedMcAccountLogin};

#[derive(Debug)]
#[must_use]
pub struct Notifiers {
    platforms: Vec<Arc<dyn Notifier>>,
}

impl Default for Notifiers {
    fn default() -> Self {
        Self::new()
    }
}

impl Notifiers {
    pub fn new() -> Self {
        Self {
            platforms: Vec::new(),
        }
    }

    pub fn add(&mut self, notifier: Arc<impl Notifier>) -> &mut Self {
        self.platforms.push(notifier);
        self
    }
}

#[async_trait]
impl Notifier for Notifiers {
    async fn admin_used_command(&self, metadata: &CommandAlert) -> Result<(), ErasedReport> {
        let iter = self.platforms.iter();
        let futures = iter.map(|v| v.admin_used_command(metadata));
        try_join_all(futures).await.map(|_| ())
    }

    async fn guest_player_joined(&self, event: &McLoginEvent) -> Result<(), ErasedReport> {
        let iter = self.platforms.iter();
        let futures = iter.map(|v| v.guest_player_joined(event));
        try_join_all(futures).await.map(|_| ())
    }

    async fn revoked_login(&self, metadata: &LinkedMcAccountLogin) -> Result<(), ErasedReport> {
        let iter = self.platforms.iter();
        let futures = iter.map(|v| v.revoked_login(metadata));
        try_join_all(futures).await.map(|_| ())
    }
}
