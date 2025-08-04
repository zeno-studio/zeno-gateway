use axum::{
    routing::{get}, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use rustls::crypto::CryptoProvider;
use std::net::SocketAddr;
use tonic::transport::Server;

mod api;
mod appstate;
mod common;
mod endpoint;
mod filter;
mod forex;
mod indexer;
mod prometheus;
mod rpc;

use filter::health_check;
use forex::{update_latest_forex_data};
use prometheus::metrics_handler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化默认加密提供者
    CryptoProvider::install_default(rustls::crypto::ring::default_provider())
        .map_err(|_| "Failed to install default crypto provider".to_string())?;

    // 初始化环境变量
    dotenvy::dotenv().map_err(|e| format!("Failed to load .env file: {}", e))?;

    // 初始化 AppState
    let state = appstate::init_app_state()
        .await
        .map_err(|e| format!("Failed to initialize app state: {}", e))?;

    // 启动外汇数据更新任务
    let forex_state = state.clone();
    tokio::spawn(async move {
        update_latest_forex_data(forex_state).await;
    });

    // 克隆状态用于 gRPC 服务
    let rpc_state = state.clone();
    let indexer_state = state.clone();
    let forex_grpc_state = state.clone();

    // 启动 gRPC 服务器
    tokio::spawn(async move {
        let rpc_service = rpc::GrpcService { state: rpc_state };
        let indexer_service = indexer::GrpcService { state: indexer_state };
        let forex_service = forex::GrpcService { state: forex_grpc_state };

        let addr = "0.0.0.0:50051".parse().unwrap();
        println!("Starting gRPC server on {}", addr);

        Server::builder()
            .add_service(api::rpc_service_server::RpcServiceServer::new(rpc_service))
            .add_service(api::ankr_indexer_service_server::AnkrIndexerServiceServer::new(indexer_service))
            .add_service(api::forex_service_server::ForexServiceServer::new(forex_service))
            .serve(addr)
            .await
            .expect("gRPC server failed");
    });

    // 配置 Axum 路由
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .with_state(state.clone());

    // 启动 HTTP 服务器
    let port = std::env::var("HTTP_PORT").unwrap_or_else(|_| "8443".to_string());
    let addr = format!("0.0.0.0:{}", port).parse()?;
    let cert_path = std::env::var("TLS_CERT_PATH").unwrap_or_else(|_| "cert.pem".to_string());
    let key_path = std::env::var("TLS_KEY_PATH").unwrap_or_else(|_| "key.pem".to_string());
    let tls_config = RustlsConfig::from_pem_file(cert_path, key_path)
        .await
        .map_err(|e| format!("Failed to load TLS config: {}", e))?;

    println!("Starting HTTP server on https://{}", addr);
    axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .map_err(|e| format!("HTTP server failed: {}", e))?;

    Ok(())
}
