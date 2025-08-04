use prometheus::{Encoder, TextEncoder, CounterVec, HistogramVec};
use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    response::Response,
};
use tokio::time::Instant;
use crate::appstate::AppState;

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

pub async fn metrics_middleware(
    State(state): State<AppState>,
    req: Request,
    next: axum::middleware::Next,
) -> Response {
    let start = Instant::now();
    let path = req.uri().path();
    let method = req.method().as_str();

    if !path.starts_with("/metrics") {
        state.metrics.http_requests_total.with_label_values(&[path, method, "pending"]).inc();
    }

    let path_clone = path.to_owned();
    let method_clone = method.to_owned();
    let response = next.run(req).await;

    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();
    state.metrics.http_request_duration.with_label_values(&[&path_clone, &method_clone, &status]).observe(duration);
    state.metrics.http_requests_total.with_label_values(&[&path_clone, &method_clone, &status]).inc();
    response
}


#[derive(Debug, Clone)]
pub struct PrometheusMetrics {
    pub http_requests_total: CounterVec,
    pub http_request_duration: HistogramVec,
    pub grpc_requests_total: CounterVec,
    pub grpc_request_duration: HistogramVec,
    pub rate_limit_exceeded_total: CounterVec, // 新增
    pub registry: prometheus::Registry,
}

impl PrometheusMetrics {
    pub fn new() -> Self {
        let http_requests_total = CounterVec::new(
            prometheus::Opts::new("http_requests_total", "Total number of HTTP requests"),
            &["path", "method", "status"],
        ).unwrap();
        let http_request_duration = HistogramVec::new(
            prometheus::HistogramOpts::new("http_request_duration_seconds", "HTTP request duration")
                .buckets(vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
            &["path", "method", "status"],
        ).unwrap();
        let grpc_requests_total = CounterVec::new(
            prometheus::Opts::new("grpc_requests_total", "Total number of gRPC requests"),
            &["service", "method", "status"],
        ).unwrap();
        let grpc_request_duration = HistogramVec::new(
            prometheus::HistogramOpts::new("grpc_request_duration_seconds", "gRPC request duration")
                .buckets(vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
            &["service", "method", "status"],
        ).unwrap();
        let rate_limit_exceeded_total = CounterVec::new(
            prometheus::Opts::new("rate_limit_exceeded_total", "Total number of requests exceeding rate limit"),
            &["path", "hash"],
        ).unwrap();

        let registry = prometheus::Registry::new();
        registry.register(Box::new(http_requests_total.clone())).unwrap();
        registry.register(Box::new(http_request_duration.clone())).unwrap();
        registry.register(Box::new(grpc_requests_total.clone())).unwrap();
        registry.register(Box::new(grpc_request_duration.clone())).unwrap();
        registry.register(Box::new(rate_limit_exceeded_total.clone())).unwrap();

        Self {
            http_requests_total,
            http_request_duration,
            grpc_requests_total,
            grpc_request_duration,
            rate_limit_exceeded_total,
            registry,
        }
    }
}
