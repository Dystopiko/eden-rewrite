pub mod context;
pub mod internal_alerts;
pub mod notification;
pub mod processing;

pub use self::context::JobContext;

use crate::{
    internal_alerts::{AlertCommandJob, AlertGuestJoinedJob, AlertRevokedLoginJob},
    notification::NotifyPendingLoginJob,
};
use eden_background_worker::runner::Runner;
use std::{sync::Arc, time::Duration};

pub trait RunnerExt {
    fn register_eden_job_types(self) -> Self;
}

impl RunnerExt for Runner<Arc<JobContext>> {
    fn register_eden_job_types(self) -> Self {
        self.configure_queue("general", |q| {
            q.register::<AlertCommandJob>()
                .register::<AlertGuestJoinedJob>()
                .register::<AlertRevokedLoginJob>()
                .register::<NotifyPendingLoginJob>()
                .poll_interval(Duration::from_secs(1))
                .workers(1)
        })
    }
}
