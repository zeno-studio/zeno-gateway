use prometheus::{CounterVec, HistogramVec};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use reqwest::Client;

#[derive(Clone, Debug)]
pub struct AppState {
    pub ankr_key: String,
    pub blast_key: String,
    pub openexchange_key: String,
    pub forex_data: Arc<RwLock<ForexData>>,
    pub rpc_endpoints: HashMap<String, String>,
    pub indexer_endpoints: HashMap<String, String>,
    pub metrics: PrometheusMetrics,
    pub client: Client,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ForexData {
    pub timestamp: u64,
    pub rates: std::collections::HashMap<String, f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawForexData {
    pub timestamp: u64,
    pub rates: std::collections::HashMap<String, f64>,
}



#[derive(Debug, Clone,)]
pub struct PrometheusMetrics {
    pub http_requests_total: CounterVec,
    pub http_request_duration: HistogramVec,
    pub registry: prometheus::Registry,
}

impl PrometheusMetrics {
    pub fn new() -> Self {
        let http_requests_total = CounterVec::new(
            prometheus::Opts::new(
                "http_requests_total",
                "Total number of HTTP requests",
            ),
            &["path", "method", "status"],
        )
        .unwrap();
        let http_request_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "http_request_duration_seconds",
                "HTTP request duration in seconds",
            )
            .buckets(vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
            &["path", "method", "status"],
        )
        .unwrap();

        let registry = prometheus::Registry::new();
        registry.register(Box::new(http_requests_total.clone())).unwrap();
        registry.register(Box::new(http_request_duration.clone())).unwrap();

        Self {
            http_requests_total,
            http_request_duration,
            registry,
        }
    }
}
