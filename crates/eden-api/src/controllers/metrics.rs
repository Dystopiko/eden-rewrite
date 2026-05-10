use axum::extract::State;
use eden_api_types::error::ErrorCode;
use eden_metrics::MetricsAdapter;
use std::sync::Arc;

use crate::{context::WebContext, controllers::ApiResult, error::ApiError};

pub async fn get(ctx: State<Arc<WebContext>>) -> ApiResult<String> {
    let Some(metrics) = ctx.metrics.as_ref() else {
        return Err(ApiError::from_static(
            ErrorCode::NotFound,
            "Metrics are disabled in this instance",
        ));
    };

    refresh_pool_stats("primary", ctx.pools.primary_db(), &**metrics);
    if let Some(replica) = ctx.pools.replica_db() {
        refresh_pool_stats("replica", replica, &**metrics);
    }

    Ok(metrics.encode_to_http()?)
}

fn refresh_pool_stats(key: &str, pool: &eden_postgres::Pool, metrics: &dyn MetricsAdapter) {
    metrics.record_db_idle_connections(key, u32::try_from(pool.idle_connections()).unwrap_or(0));
}

#[cfg(test)]
mod tests {
    use eden_metrics::MockMetricsAdapter;

    use crate::testing::{TestApp, assert_response, setup_for_route};

    #[sqlx::test]
    async fn should_reject_request_if_metrics_is_disabled(pool: sqlx::PgPool) {
        let _guard = setup_for_route!["metrics", "GET"];

        let (_, server) = TestApp::builder(pool).build();
        let response = server.get("/metrics").await;
        assert_response!(response as str);
    }

    #[sqlx::test]
    async fn should_provide_metrics_if_metrics_is_enabled(pool: sqlx::PgPool) {
        let mut metrics = MockMetricsAdapter::new();
        metrics
            .expect_record_db_idle_connections()
            .times(1)
            .returning(|_, _| {});

        metrics
            .expect_encode_to_http()
            .returning(|| Ok("".to_string()));

        let (_, server) = TestApp::builder(pool).with_metrics(metrics).build();
        server
            .get("/metrics")
            .await
            .assert_text("")
            .assert_status_ok();
    }
}
