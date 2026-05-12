pub mod auth;
pub mod controllers;
pub mod convert;
pub mod error;
pub mod extract;

pub use self::error::ApiError;

use eden_common::AppContext;
use std::{ops::Deref, sync::Arc};

#[derive(Debug)]
pub struct WebContext {
    pub app: Arc<AppContext>,
}

impl Deref for WebContext {
    type Target = AppContext;

    fn deref(&self) -> &Self::Target {
        &self.app
    }
}
