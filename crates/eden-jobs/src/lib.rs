pub mod alerts;
pub mod context;
pub mod events;
pub mod notification;

pub use self::context::JobContext;

use crate::{
    alerts::{AlertCommandJob, AlertGuestJoinedJob},
    events::OnPlayerJoinedJob,
    notification::pending_ip_login::NotifyPendingIpLogin,
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
                .register::<OnPlayerJoinedJob>()
                .register::<NotifyPendingIpLogin>()
                .poll_interval(Duration::from_secs(1))
                .workers(1)
        })
    }
}
