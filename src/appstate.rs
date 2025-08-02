use prometheus::{CounterVec, HistogramVec};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use reqwest::Client;
use tonic::transport::Channel;
use crate::api::{
    forex_service_client::ForexServiceClient,
    rpc_service_client::RpcServiceClient,
    indexer_service_client::IndexerServiceClient,
};



#[derive(Clone, Debug)]
pub struct AppState {
    pub ankr_key: String,
    pub blast_key: String,
    pub openexchange_key: String,
    pub forex_data: Arc<RwLock<ForexData>>,
    pub raw_forex_data: Arc<RwLock<Option<RawForexData>>>,
    pub rpc_endpoints: HashMap<String, String>,
    pub indexer_endpoints: HashMap<String, String>,
    pub metrics: PrometheusMetrics,
    pub client: Client,
    pub forex_client: ForexServiceClient<Channel>,
    pub rpc_client: RpcServiceClient<Channel>,
    pub indexer_client: IndexerServiceClient<Channel>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ForexData {
    pub timestamp: u64,
    pub rates: std::collections::HashMap<String, f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawForexData {
    pub disclaimer: String,
    pub license: String,
    pub timestamp: u64,
    pub base: String,
    pub rates: std::collections::HashMap<String, f64>,
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