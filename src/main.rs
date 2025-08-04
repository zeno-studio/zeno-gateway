use axum::{Router, middleware};
use axum_server::tls_rustls::RustlsConfig;
use reqwest::Client;
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, error, info};
use tracing_subscriber::{filter::EnvFilter, fmt, util::SubscriberInitExt};

mod appstate;
use appstate::{AppState, ForexData, PrometheusMetrics};

mod endpoint;
use endpoint::{
    indexer_proxy, rpc_proxy, setup_ankr_endpoints, setup_blast_endpoints, setup_indexer_endpoints,
};

mod prometheus;
use prometheus::{metrics_handler, metrics_middleware};

mod forex;
use forex::{get_forex_data, update_forex_data};

mod filter;
use filter::{add_headers, health_check};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing
    fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("info,axum=debug,tower_http=debug")
        }))
        .with_target(true)
        .with_thread_ids(true)
        .finish()
        .init();
    info!("Logging initialized with tracing");

    // Initialize rustls
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|e| format!("Failed to install rustls crypto provider: {:?}", e))?;

    // Load environment variables
    dotenvy::dotenv().map_err(|e| format!("Failed to load .env file: {}", e))?;
    info!("Loaded environment variables");

    // Retrieve API keys
    let ankr_key = env::var("ANKR_API_KEY").unwrap_or_default();
    let blast_key = env::var("BLAST_API_KEY").unwrap_or_default();
    let openexchange_key = env::var("OPENEXCHANGE_KEY").unwrap_or_default();
    info!(
        "Retrieved API keys: ankr_key={} (len={}), blast_key={} (len={}), openexchange_key=masked (len={})",
        if ankr_key.is_empty() { "empty" } else { "set" },
        ankr_key.len(),
        if blast_key.is_empty() { "empty" } else { "set" },
        blast_key.len(),
        openexchange_key.len()
    );

    // Initialize endpoints
    let mut rpc_endpoints = HashMap::new();
    let mut indexer_endpoints = HashMap::new();
    setup_ankr_endpoints(&mut rpc_endpoints, &ankr_key);
    setup_blast_endpoints(&mut rpc_endpoints, &blast_key);
    setup_indexer_endpoints(&mut indexer_endpoints, &ankr_key);
    info!(
        "Initialized {} RPC endpoints: {:?}, {} indexer endpoints: {:?}",
        rpc_endpoints.len(),
        rpc_endpoints.keys().collect::<Vec<_>>(),
        indexer_endpoints.len(),
        indexer_endpoints.keys().collect::<Vec<_>>()
    );

    // Initialize reqwest client
    let client = Client::builder()
        .use_rustls_tls()
        .pool_max_idle_per_host(10)
        .http2_keep_alive_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(10))
        .gzip(true)
        .brotli(true)
        .build()
        .map_err(|e| format!("Failed to build reqwest client: {}", e))?;
    info!("Built reqwest client with rustls TLS");

    // Initialize application state
    let state = AppState {
        ankr_key,
        blast_key,
        openexchange_key,
        forex_data: Arc::new(RwLock::new(ForexData {
            timestamp: 0,
            rates: HashMap::new(),
        })),
        rpc_endpoints,
        indexer_endpoints,
        metrics: PrometheusMetrics::new(),
        client,
    };
    info!("Initialized application state");

    // Spawn forex update task
    tokio::spawn(update_forex_data(state.clone()));
    info!("Spawned forex data update task");

    // Setup rate limit layers
    let rpc_governor_conf = tower_governor::governor::GovernorConfigBuilder::default()
        .per_second(filter::RPC_RATE_LIMIT)
        .burst_size(filter::RPC_BURST_SIZE as u32)
        .key_extractor(filter::DeviceFingerprintKeyExtractor)
        .finish()
        .unwrap();
    let rpc_rate_limit_layer = tower_governor::GovernorLayer {
        config: std::sync::Arc::new(rpc_governor_conf),
    };

    let indexer_governor_conf = tower_governor::governor::GovernorConfigBuilder::default()
        .per_second(filter::INDEXER_RATE_LIMIT)
        .burst_size(filter::INDEXER_BURST_SIZE as u32)
        .key_extractor(filter::DeviceFingerprintKeyExtractor)
        .finish()
        .unwrap();
    let indexer_rate_limit_layer = tower_governor::GovernorLayer {
        config: std::sync::Arc::new(indexer_governor_conf),
    };

    let forex_governor_conf = tower_governor::governor::GovernorConfigBuilder::default()
        .per_second(filter::FOREX_RATE_LIMIT)
        .burst_size(filter::FOREX_BURST_SIZE as u32)
        .key_extractor(filter::DeviceFingerprintKeyExtractor)
        .finish()
        .unwrap();
    let forex_rate_limit_layer = tower_governor::GovernorLayer {
        config: std::sync::Arc::new(forex_governor_conf),
    };

    let health_governor_conf = tower_governor::governor::GovernorConfigBuilder::default()
        .per_second(filter::HEALTH_RATE_LIMIT)
        .burst_size(filter::HEALTH_BURST_SIZE as u32)
        .key_extractor(filter::DeviceFingerprintKeyExtractor)
        .finish()
        .unwrap();
    let health_rate_limit_layer = tower_governor::GovernorLayer {
        config: std::sync::Arc::new(health_governor_conf),
    };

    // Define routes
    let app = Router::new()
        .route("/rpc/{provider}/{chain}", axum::routing::get(rpc_proxy).post(rpc_proxy))
        .route("/indexer/{provider}", axum::routing::get(indexer_proxy).post(indexer_proxy))
        .route("/forex", axum::routing::get(get_forex_data))
        .route("/health", axum::routing::get(health_check))
        .route("/metrics", axum::routing::get(metrics_handler))
        .with_state(state.clone())
        .layer(rpc_rate_limit_layer)
        .layer(indexer_rate_limit_layer)
        .layer(forex_rate_limit_layer)
        .layer(health_rate_limit_layer)
        .layer(middleware::from_fn_with_state(state.clone(), metrics_middleware))
        .layer(middleware::from_fn_with_state(state.clone(), add_headers))
        .layer(middleware::from_fn(filter::redirect_to_https))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().include_headers(true))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        );

    // Load TLS certificates
    let cert_path = env::var("TLS_CERT_PATH").unwrap_or("./cert.pem".to_string());
    let key_path = env::var("TLS_KEY_PATH").unwrap_or("./key.pem".to_string());
    let tls_config = RustlsConfig::from_pem_file(&cert_path, &key_path)
        .await
        .map_err(|e| {
            error!(
                "Failed to load TLS certificates: cert={}, key={}, error={}",
                cert_path, key_path, e
            );
            format!("Failed to load TLS certificates: {}", e)
        })?;

    // Start HTTPS server
    info!("Starting HTTPS server on 0.0.0.0:8443");
    axum_server::bind_rustls("0.0.0.0:8443".parse()?, tls_config)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;

    Ok(())
}
