use prometheus::{Encoder, TextEncoder, CounterVec, HistogramVec};
use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    response::Response,
};
use tokio::time::Instant;

use crate::appstate::AppState;



// Prometheus metrics handler
pub async fn metrics_handler(State(state): State<AppState>) -> Response {
    let encoder = TextEncoder::new();
    let metric_families = state.metrics.registry.gather();

    match encoder.encode_to_string(&metric_families) {
        Ok(output) => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", encoder.format_type())
            .body(Body::from(output))
            .unwrap(),
        Err(e) => {
            println!("Failed to encode Prometheus metrics: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("Failed to encode metrics: {}", e)))
                .unwrap()
        }
    }
}

// Prometheus metrics middleware
pub async fn metrics_middleware(
    State(state): State<AppState>,
    req: Request,
    next: axum::middleware::Next,
) -> Response {
    let start = Instant::now();
    let path = req.uri().path();
    let method = req.method().as_str();

    // Increment request counter (exclude /metrics endpoint)
    if !path.starts_with("/metrics") {
        state.metrics.http_requests_total.with_label_values(&[path, method, "pending"]).inc();
    }

    // Clone the parts we need before consuming the request
    let path_clone = path.to_owned();
    let method_clone = method.to_owned();
    
    // Run the next handler, consuming req
    let response = next.run(req).await;

    // Record request duration
    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();
    state.metrics.http_request_duration.with_label_values(&[&path_clone, &method_clone, &status]).observe(duration);
    response
}