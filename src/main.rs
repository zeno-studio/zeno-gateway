// src/main.rs
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Router,
    extract::State,
    routing::{get, post},
    http::StatusCode,
    response::IntoResponse,
};
use axum_server::tls_rustls::RustlsConfig;
use tonic::transport::Server;

use crate::{
    state::AppState,
    pb::ankr::ankr_indexer_server::{AnkrIndexerServer},
    ankr::IndexService,
};

mod db;
mod state;
mod ankr;
mod ankr_types;
mod pb; // tonic 生成的代码在 pb/ 目录

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 初始化日志（建议用 tracing + tracing_subscriber）
    tracing_subscriber::fmt::init();
    let state = Arc::new(AppState::new());

    // ==================== gRPC 服务 ====================
    let grpc_addr: SocketAddr = "0.0.0.0:50052".parse()?;  // 更改端口避免冲突
    let indexer_service = IndexService { state: state.clone() };

    let grpc_router = Server::builder()
        .add_service(AnkrIndexerServer::new(indexer_service));

    // ==================== Axum HTTPS JSON Gateway ====================
    let axum_app = Router::new()
        .route("/health", get(health_check))
        // 下面这两个路由把 Ankr Multichain 的 JSON 接口直接代理过去（方便前端直接调用）
        .route("/ankr_indexer", post(ankr_indexer_post))
        .route("/ankr_indexer", get(|| async { (
            StatusCode::METHOD_NOT_ALLOWED,
            "Use POST for ankr_indexer"
        )}))
        .with_state(state.clone());

    // TLS 配置（和原来完全一样）
    let cert_path = std::env::var("TLS_CERT_PATH").unwrap_or("./cert.pem".to_string());
    let key_path = std::env::var("TLS_KEY_PATH").unwrap_or("./key.pem".to_string());

    let https_addr: SocketAddr = "0.0.0.0:8444".parse()?;  // 更改端口避免冲突

    let grpc_handle = tokio::spawn(async move {
        grpc_router
            .serve(grpc_addr)
            .await
            .expect("gRPC server crashed");
    });

    // 尝试加载 TLS 证书，如果失败则使用 HTTP
    let server_handle = match RustlsConfig::from_pem_file(&cert_path, &key_path).await {
        Ok(tls_config) => {
            tokio::spawn(async move {
                axum_server::bind_rustls(https_addr, tls_config)
                    .serve(axum_app.into_make_service_with_connect_info::<SocketAddr>())
                    .await
                    .expect("HTTPS server crashed");
            })
        }
        Err(e) => {
            tracing::warn!("Failed to load TLS cert/key: {}. Falling back to HTTP on port 8080", e);
            let http_addr: SocketAddr = "0.0.0.0:8080".parse().expect("Invalid HTTP address");
            tokio::spawn(async move {
                axum_server::bind(http_addr)
                    .serve(axum_app.into_make_service_with_connect_info::<SocketAddr>())
                    .await
                    .expect("HTTP server crashed");
            })
        }
    };

    // 等待任意一个崩溃就退出
    tokio::select! {
        _ = grpc_handle => tracing::error!("gRPC server stopped"),
        _ = server_handle => tracing::error!("HTTP/HTTPS server stopped"),
    }

    Ok(())
}

// ==================== Axum Handler（直接透传到 Ankr）====================
async fn ankr_indexer_post(
    State(state): State<Arc<AppState>>,
    body: axum::extract::Json<serde_json::Value>,
) -> impl IntoResponse {
    let endpoint = format!("https://rpc.ankr.com/multichain/{}", state.ankr_key);

    let client = &state.client;
    let resp = client
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .json(&body.0)
        .send()
        .await;

    match resp {
        Ok(r) => {
            let status = r.status();
            let text = r.text().await.unwrap_or_default();
            (axum::http::StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), text)
        }
        Err(e) => {
            tracing::error!("Ankr request failed: {e}");
            (StatusCode::BAD_GATEWAY, format!("Ankr error: {e}"))
        }
    }
}

async fn health_check() -> &'static str {
    "OK"
}