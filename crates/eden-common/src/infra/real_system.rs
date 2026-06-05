use crate::domain::System;

use async_trait::async_trait;
use erased_report::{EraseReportExt, ErasedReport};
use error_stack::ResultExt;
use thiserror::Error;
use tokio::task::spawn_blocking;

#[derive(Debug)]
pub struct RealSystem;

#[derive(Debug, Error)]
#[error("Failed to retrieve system's hostname")]
pub struct GetHostnameError;

#[async_trait]
impl System for RealSystem {
    async fn hostname(&self) -> Result<String, ErasedReport> {
        spawn_blocking(|| {
            hostname::get()
                .change_context(GetHostnameError)
                .map(|v| v.to_string_lossy().to_string())
        })
        .await
        .change_context(GetHostnameError)
        .flatten()
        .erase_report()
    }
}
