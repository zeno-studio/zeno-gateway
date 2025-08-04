use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use reqwest::Client;

use crate::endpoint;

use crate::prometheus::PrometheusMetrics;



#[derive(Clone, Debug)]
pub struct AppState {
    pub ankr_key: String,
    pub blast_key: String,
    pub openexchange_key: String,
    pub latest_forex_data: Arc<RwLock<ForexData>>,
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
    pub disclaimer: String,
    pub license: String,
    pub timestamp: u64,
    pub base: String,
    pub rates: std::collections::HashMap<String, f64>,
}

pub async fn init_app_state() -> Result<AppState, Box<dyn std::error::Error>> {
     let client = Client::builder()
        .use_rustls_tls()
        .build()
        .map_err(|e| {
            eprintln!("Failed to create HTTP client: {}", e);
            e
        })?;
    let ankr_key = std::env::var("ANKR_API_KEY").unwrap_or_default();
    let blast_key = std::env::var("BLAST_API_KEY").unwrap_or_default();
    let openexchange_key = std::env::var("OPENEXCHANGE_KEY").unwrap_or_default();

    let mut rpc_endpoints = HashMap::new();
    let mut indexer_endpoints = HashMap::new();
    endpoint::setup_ankr_endpoints(&mut rpc_endpoints, &ankr_key);
    endpoint::setup_blast_endpoints(&mut rpc_endpoints, &blast_key);
    endpoint::setup_indexer_endpoints(&mut indexer_endpoints, &ankr_key);

    Ok(AppState {
        ankr_key,
        blast_key,
        openexchange_key,
        latest_forex_data: Arc::new(RwLock::new(ForexData { timestamp: 0, rates: HashMap::new() })),
        rpc_endpoints,
        indexer_endpoints,
        metrics: crate::prometheus::PrometheusMetrics::new(),
        client,
    })
}
