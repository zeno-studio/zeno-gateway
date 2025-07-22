// Prometheus 相关导入

use prometheus::{Encoder, TextEncoder};

use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    response::Response,
};

use crate::appstate::AppState;
use tokio::time::Instant;

// Prometheus 指标端点
pub async fn metrics_handler(State(state): State<AppState>) -> Response {
    let encoder = TextEncoder::new();
    let metric_families = state.metrics.registry.gather();

    match encoder.encode_to_string(&metric_families) {
        Ok(output) => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", encoder.format_type())
            .body(Body::from(output))
            .unwrap(),
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Failed to encode metrics"))
            .unwrap(),
    }
}

pub async fn metrics_middleware(
    State(state): State<AppState>,
    req: Request,
    next: axum::middleware::Next,
) -> Response {
    let start = Instant::now();
    let path = req.uri().path().to_string();

    // 增加HTTP请求计数
    state.metrics.http_requests_total.inc();

    // 如果是RPC请求，也增加RPC计数
    if path.starts_with("/rpc/") {
        state.metrics.rpc_requests_total.inc();
    }

    // 如果是Indexer请求，也增加Indexer计数
    if path.starts_with("/indexer") {
        state.metrics.indexer_requests_total.inc();
    }

    let response = next.run(req).await;

    // 记录请求持续时间
    let duration = start.elapsed().as_secs_f64();
    state.metrics.http_request_duration.observe(duration);

    if path.starts_with("/rpc/") {
        state.metrics.rpc_request_duration.observe(duration);
    }

    if path.starts_with("/indexer") {
        state.metrics.indexer_request_duration.observe(duration);
    }

    response
}
