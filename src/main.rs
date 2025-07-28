use axum::{Router, middleware, routing::get};
use axum_server::tls_rustls::RustlsConfig;
use reqwest::Client;
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_governor::{
    GovernorLayer, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor,
};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, error, info};
use tracing_subscriber::{fmt, filter::EnvFilter};


mod appstate;
use appstate::{AppState, ForexData, PrometheusMetrics};

mod endpoint;
use endpoint::{
    indexer_proxy, rpc_proxy, setup_ankr_endpoints, setup_blast_endpoints, setup_indexer_endpoints,
};

mod prometheus;
use prometheus::{metrics_handler, metrics_middleware};

mod forex;
use forex::{get_forex_data, get_raw_forex_data, update_forex_data};

mod filter;
use filter::{add_headers, domain_filter, health_check};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let subscriber = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,axum=debug,tower_http=debug")),
        )
        .with_target(true)
        .with_thread_ids(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    info!("Logging initialized with tracing");

    dotenvy::dotenv().map_err(|e| format!("Failed to load .env file: {}", e))?;
    info!("Loaded environment variables");

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

    let client = Client::builder()
    .build()
    .map_err(|e| format!("Failed to build reqwest client: {}", e))?;
    info!("Built reqwest client with rustls TLS");

    let state = AppState {
        ankr_key,
        blast_key,
        openexchange_key,
        forex_data: Arc::new(RwLock::new(ForexData {
            timestamp: 0,
            rates: HashMap::new(),
        })),
        raw_forex_data: Arc::new(RwLock::new(None)),
        rpc_endpoints,
        indexer_endpoints,
        metrics: PrometheusMetrics::new(),
        client,
    };
    info!("Initialized application state");

    tokio::spawn(update_forex_data(state.clone()));
    info!("Spawned forex data update task");

    let rpc_governor_conf = GovernorConfigBuilder::default()
        .per_second(30)
        .burst_size(30)
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .unwrap();
    let rpc_rate_limit_layer = ServiceBuilder::new().layer(GovernorLayer {
        config: Arc::new(rpc_governor_conf),
    });

    let indexer_governor_conf = GovernorConfigBuilder::default()
        .per_second(10)
        .burst_size(10)
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .unwrap();
    let indexer_rate_limit_layer = ServiceBuilder::new().layer(GovernorLayer {
        config: Arc::new(indexer_governor_conf),
    });

    let forex_governor_conf = GovernorConfigBuilder::default()
        .per_second(5)
        .burst_size(5)
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .unwrap();
    let forex_rate_limit_layer = ServiceBuilder::new().layer(GovernorLayer {
        config: Arc::new(forex_governor_conf),
    });

    let health_governor_conf = GovernorConfigBuilder::default()
        .per_second(3)
        .burst_size(3)
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .unwrap();
    let health_rate_limit_layer = ServiceBuilder::new().layer(GovernorLayer {
        config: Arc::new(health_governor_conf),
    });

    let rpc_routes = Router::new()
        .route("/rpc/{provider}/{chain}", get(rpc_proxy).post(rpc_proxy))
        .with_state(state.clone())
        .layer(rpc_rate_limit_layer);

    let indexer_routes = Router::new()
        .route(
            "/indexer/{provider}",
            get(indexer_proxy).post(indexer_proxy),
        )
        .with_state(state.clone())
        .layer(indexer_rate_limit_layer);

    let forex_routes = Router::new()
        .route("/forex", get(get_forex_data))
        .route("/forex/raw", get(get_raw_forex_data))
        .with_state(state.clone())
        .layer(forex_rate_limit_layer);

    let health_routes = Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .with_state(state.clone())
        .layer(health_rate_limit_layer);

    let app = Router::new()
        .merge(rpc_routes)
        .merge(indexer_routes)
        .merge(forex_routes)
        .merge(health_routes)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            metrics_middleware,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), add_headers))
        .layer(middleware::from_fn(domain_filter))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().include_headers(true))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        );

    let cert_path_str = env::var("TLS_CERT_PATH").unwrap_or("./cert.pem".to_string());
    let key_path_str = env::var("TLS_KEY_PATH").unwrap_or("./key.pem".to_string());

    let cert_path = Path::new(&cert_path_str);
    let key_path = Path::new(&key_path_str);

    if cert_path.is_file() && key_path.is_file() {
        match std::fs::metadata(cert_path).and_then(|_| std::fs::metadata(key_path)) {
            Ok(_) => match RustlsConfig::from_pem_file(cert_path, key_path).await {
                Ok(tls_config) => {
                    info!(user = ?env::var("USER"), "TLS certificates loaded successfully");
                    info!("Certificate: {}", cert_path.display());
                    info!("Private Key: {}", key_path.display());
                    info!("Server running on https://0.0.0.0:8443");

                    axum_server::bind_rustls("0.0.0.0:8443".parse()?, tls_config)
                        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
                        .await?;
                }
                Err(e) => {
                    error!("Failed to load TLS certificates: {}", e);
                    info!("Falling back to HTTP server on http://0.0.0.0:3000...");
                    start_http_server(app).await;
                }
            },
            Err(e) => {
                error!("Cannot access certificate or key files: {}", e);
                info!("Certificate: {}", cert_path.display());
                info!("Private Key: {}", key_path.display());
                info!("Falling back to HTTP server on http://0.0.0.0:3000...");
                start_http_server(app).await;
            }
        }
    } else {
        error!("Certificate or key file not found");
        if !cert_path.is_file() {
            error!("Certificate: {} (not a file)", cert_path.display());
        }
        if !key_path.is_file() {
            error!("Private Key: {} (not a file)", key_path.display());
        }
        info!("Falling back to HTTP server on http://0.0.0.0:3000...");
        start_http_server(app).await;
    }

    Ok(())
}

async fn start_http_server(app: Router) {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("Server running on http://0.0.0.0:3000");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
