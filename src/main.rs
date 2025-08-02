use axum::{
    middleware,
    routing::{get, post},
    Router,
    extract::ConnectInfo,
};
use axum_server::tls_rustls::RustlsConfig;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use reqwest::Client;
use tower_http::limit::RequestBodyLimitLayer;
use tracing_subscriber;

mod appstate;
mod endpoint;
mod prometheus;
mod filter;
mod forex;
mod backend;
mod api;

use appstate::{AppState, ForexData};
use endpoint::{rpc_proxy, indexer_proxy};
use filter::{add_headers, health_check, rate_limit_middleware};
use forex::{get_forex_data, get_raw_forex_data, update_forex_data};
use prometheus::metrics_handler;

async fn init_app_state() -> AppState {
    let ankr_key = std::env::var("ANKR_API_KEY").unwrap_or_default();
    let blast_key = std::env::var("BLAST_API_KEY").unwrap_or_default();
    let openexchange_key = std::env::var("OPENEXCHANGE_KEY").unwrap_or_default();

    let mut rpc_endpoints = HashMap::new();
    let mut indexer_endpoints = HashMap::new();
    endpoint::setup_ankr_endpoints(&mut rpc_endpoints, &ankr_key);
    endpoint::setup_blast_endpoints(&mut rpc_endpoints, &blast_key);
    endpoint::setup_indexer_endpoints(&mut indexer_endpoints, &ankr_key);

    let forex_client = api::forex_service_client::ForexServiceClient::connect("http://backend:50051")
        .await
        .unwrap();
    let rpc_client = api::rpc_service_client::RpcServiceClient::connect("http://backend:50051")
        .await
        .unwrap();
    let indexer_client = api::indexer_service_client::IndexerServiceClient::connect("http://backend:50051")
        .await
        .unwrap();

    AppState {
        ankr_key,
        blast_key,
        openexchange_key,
        forex_data: Arc::new(RwLock::new(ForexData { timestamp: 0, rates: HashMap::new() })),
        raw_forex_data: Arc::new(RwLock::new(None)),
        rpc_endpoints,
        indexer_endpoints,
        metrics: appstate::PrometheusMetrics::new(),
        client: Client::new(),
        forex_client,
        rpc_client,
        indexer_client,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let state = init_app_state().await;

    // 启动外汇数据更新任务
    let forex_state = state.clone();
    tokio::spawn(async move {
        update_forex_data(forex_state).await;
    });

    // 启动 gRPC 后端服务
    let backend_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = backend::start_backend(backend_state).await {
            eprintln!("gRPC backend failed: {}", e);
        }
    });

    // 配置 Axum 路由
    let app = Router::new()
        .route("/rpc/:provider/:chain", post(rpc_proxy))
        .route("/indexer/:provider", post(indexer_proxy))
        .route("/forex", get(get_forex_data))
        .route("/forex/raw", get(get_raw_forex_data))
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit_middleware))
        .layer(middleware::from_fn_with_state(state.clone(), add_headers))
        .layer(middleware::from_fn_with_state(state.clone(), prometheus::metrics_middleware))
        .layer(RequestBodyLimitLayer::new(1_000_000)) // 修复为 RequestBodyLimiter
        .with_state(state);

    // 启动 Axum 服务器
    let addr = "0.0.0.0:8443".parse()?;
    let tls_config = RustlsConfig::from_pem_file("cert.pem", "key.pem").await?;
    println!("Starting Axum server on {}", addr);
    axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>()) // 启用 ConnectInfo
        .await?;

    Ok(())
}