pub mod auth;
pub mod controllers;
pub mod convert;
pub mod error;
pub mod extract;
pub mod middleware;
pub mod router;

pub use self::error::ApiError;

use axum_server::tls_rustls::{RustlsAcceptor, RustlsConfig};
use eden_common::AppContext;
use error_stack::{Report, ResultExt};
use std::{net::SocketAddr, ops::Deref, sync::Arc};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("API server error")]
pub struct ApiServerError;

pub async fn service(
    ctx: Arc<AppContext>,
    config: &eden_config::types::Server,
) -> Result<(), Report<ApiServerError>> {
    // axum_server only accepts std's TcpListener
    let listener = tokio::net::TcpListener::bind((config.ip, config.port))
        .await
        .and_then(|v| v.into_std())
        .change_context(ApiServerError)?;

    let web_ctx = Arc::new(WebContext { app: ctx.clone() });
    let router = self::router::build(web_ctx);

    let addr = listener.local_addr().change_context(ApiServerError)?;
    let make_service = router.into_make_service_with_connect_info::<SocketAddr>();
    let handle = axum_server::Handle::new();

    let server = handle.clone();
    tokio::spawn(async move {
        ctx.shutdown_signal().subscribe().await;
        server.graceful_shutdown(None);
    });

    let builder = axum_server::from_tcp(listener)
        .change_context(ApiServerError)?
        .handle(handle);

    if let Some(tls) = config.tls.as_ref() {
        let rustls_config = RustlsConfig::from_pem_file(&tls.cert_file, &tls.priv_key_file)
            .await
            .change_context(ApiServerError)?;

        tracing::info!("listening at https://{addr}");

        let acceptor = RustlsAcceptor::new(rustls_config);
        builder.acceptor(acceptor).serve(make_service).await
    } else {
        tracing::info!("listening at http://{addr}");
        builder.serve(make_service).await
    }
    .change_context(ApiServerError)?;

    tracing::info!("API server has gracefully shutdown");
    Ok(())
}

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
