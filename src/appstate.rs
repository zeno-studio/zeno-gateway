
use prometheus::{
     Histogram, HistogramOpts, IntCounter, IntGauge, Opts, Registry, 
    register_histogram_with_registry,
    register_int_counter_with_registry, register_int_gauge_with_registry,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub ankr_key: String,
    pub blast_key: String,
    pub openexchange_key: String,
    pub forex_data: Arc<RwLock<ForexData>>,
    pub raw_forex_data: Arc<RwLock<Option<RawForexData>>>, // 存储原始 JSON
    pub rpc_endpoints: HashMap<String, String>,
    pub indexer_endpoints: HashMap<String, String>,
    pub metrics: PrometheusMetrics,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ForexData {
    pub timestamp: u64,
    pub rates: std::collections::HashMap<String, f64>,
}

// 原始外汇 API 响应（用于 /forex/raw）
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawForexData {
    pub disclaimer: String,
    pub license: String,
    pub timestamp: u64,
    pub base: String,
   pub  rates: std::collections::HashMap<String, f64>,
}

// Prometheus 指标结构
#[derive(Clone)]
pub struct PrometheusMetrics {
    pub registry: Registry,
    pub http_requests_total: IntCounter,
    pub http_request_duration: Histogram,
    pub rpc_requests_total: IntCounter,
    pub rpc_request_duration: Histogram,
    pub indexer_requests_total: IntCounter,
    pub indexer_request_duration: Histogram,
    pub forex_updates_total: IntCounter,
    pub active_connections: IntGauge,
    pub rate_limit_hits: IntCounter,
}

impl PrometheusMetrics {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let registry = Registry::new();

        let http_requests_total = register_int_counter_with_registry!(
            Opts::new("http_requests_total", "Total number of HTTP requests"),
            registry
        )?;

        let http_request_duration = register_histogram_with_registry!(
            HistogramOpts::new(
                "http_request_duration_seconds",
                "HTTP request duration in seconds"
            ),
            registry
        )?;

        let rpc_requests_total = register_int_counter_with_registry!(
            Opts::new("rpc_requests_total", "Total number of RPC requests"),
            registry
        )?;

        let rpc_request_duration = register_histogram_with_registry!(
            HistogramOpts::new(
                "rpc_request_duration_seconds",
                "RPC request duration in seconds"
            ),
            registry
        )?;

        let indexer_requests_total = register_int_counter_with_registry!(
            Opts::new("indexer_requests_total", "Total number of indexer requests"),
            registry
        )?;

        let indexer_request_duration = register_histogram_with_registry!(
            HistogramOpts::new(
                "indexer_request_duration_seconds",
                "Indexer request duration in seconds"
            ),
            registry
        )?;

        let forex_updates_total = register_int_counter_with_registry!(
            Opts::new("forex_updates_total", "Total number of forex data updates"),
            registry
        )?;

        let active_connections = register_int_gauge_with_registry!(
            Opts::new("active_connections", "Number of active connections"),
            registry
        )?;

        let rate_limit_hits = register_int_counter_with_registry!(
            Opts::new("rate_limit_hits_total", "Total number of rate limit hits"),
            registry
        )?;

        Ok(PrometheusMetrics {
            registry,
            http_requests_total,
            http_request_duration,
            rpc_requests_total,
            rpc_request_duration,
            indexer_requests_total,
            indexer_request_duration,
            forex_updates_total,
            active_connections,
            rate_limit_hits,
        })
    }
}
