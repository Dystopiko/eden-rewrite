use async_trait::async_trait;
use eden_model::{alerts::command::CommandAlert, tables::mc_login_event::McLoginEvent};
use erased_report::ErasedReport;
use std::sync::Arc;

use crate::domain::{Notifier, notifier::LinkedMcAccountLogin};

#[derive(Debug)]
#[must_use]
pub struct MultiPlatformNotifier {
    platforms: Vec<Arc<dyn Notifier>>,
}

impl MultiPlatformNotifier {
    pub fn new() -> Self {
        Self {
            platforms: Vec::new(),
        }
    }

    pub fn add_platform(&mut self, notifier: Arc<impl Notifier>) -> &mut Self {
        self.platforms.push(notifier);
        self
    }
}

#[async_trait]
impl Notifier for MultiPlatformNotifier {
    async fn admin_used_command(&self, metadata: &CommandAlert) -> Result<(), ErasedReport> {
        for notifier in self.platforms.iter() {
            notifier.admin_used_command(metadata).await?;
        }
        Ok(())
    }

    async fn guest_player_joined(&self, event: &McLoginEvent) -> Result<(), ErasedReport> {
        for notifier in self.platforms.iter() {
            notifier.guest_player_joined(event).await?;
        }
        Ok(())
    }

    async fn revoked_login(&self, metadata: &LinkedMcAccountLogin) -> Result<(), ErasedReport> {
        for notifier in self.platforms.iter() {
            notifier.revoked_login(metadata).await?;
        }
        Ok(())
    }
}
