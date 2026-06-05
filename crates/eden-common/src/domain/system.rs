use async_trait::async_trait;
use erased_report::ErasedReport;
use std::fmt;

#[async_trait]
#[mockall::automock]
pub trait System: fmt::Debug + Send + Sync {
    async fn hostname(&self) -> Result<String, ErasedReport>;
}
