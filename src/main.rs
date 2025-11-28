// src/main.rs
use crate::{
    client::GLOBAL_STATE,
    error::Result,
    pb::ankr::ankr_indexer_server::AnkrIndexerServer,
    rules::RateLimitInterceptor,
    state::{AppState, IndexService},
    utils::load_rustls_config,
};
use hyper::{Body, Request, Response, service::service_fn};
use rustls::ServerConfig;
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tokio::time::{Duration, interval};
use tokio_rustls::TlsAcceptor;
use tonic::transport::{Identity, Server, ServerTlsConfig};
use tonic_async_interceptor::AsyncInterceptedService; // Added for async interceptor support

mod ankr;
mod client;
mod db;
mod error;
mod pb;
mod rules;
mod state;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // 1. 证书读取
    let cert_pem = tokio::fs::read("./cert.pem").await?;
    let key_pem = tokio::fs::read("./key.pem").await?;
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    // 2. 准备服务实例
    let state = Arc::new(AppState::new());

    // 业务服务：挂载鉴权拦截器 (check JWT)
    let indexer = IndexService {
        state: state.clone(),
    };

    let rate_limit = RateLimitInterceptor { rule_name: "ankr" };

    // Changed to use AsyncInterceptedService
    let ankr_svc = AsyncInterceptedService::new(AnkrIndexerServer::new(indexer), rate_limit);
    
    // 4. 构建 gRPC 路由层
    let grpc_addr = "0.0.0.0:50051".parse()?;
    let grpc_identity = Identity::from_pem(&cert_pem, &key_pem);

    let grpc_server = Server::builder()
        .tls_config(ServerTlsConfig::new().identity(grpc_identity))?
        .add_service(ankr_svc) // 注册业务服务 (Protected)
        .serve(grpc_addr);

    // 5. Health Server (不做变动)
    let http_addr = "0.0.0.0:8443".parse()?;
    let http_tls_config = Arc::new(load_rustls_config(&cert_pem, &key_pem)?);
    let http_server = run_health_server(http_addr, http_tls_config);

    // 6. 启动心跳检测任务
    let heartbeat_server = heartbeat_task();

    println!("gRPC Server listening on {}", grpc_addr);

    tokio::try_join!(
        async { grpc_server.await.map_err(error::AppError::from) },
        async { http_server.await.map_err(error::AppError::from) },
        async { heartbeat_server.await.map_err(error::AppError::from) }
    )?;

    Ok(())
}

// --- 极简 Health Check (保留给 Cloudflare) ---
async fn health_handler(_: Request<Body>) -> std::result::Result<Response<Body>, Infallible> {
    Ok(Response::new(Body::from("OK")))
}

async fn run_health_server(addr: SocketAddr, tls_config: Arc<ServerConfig>) -> Result<()> {
    let acceptor = TlsAcceptor::from(tls_config);
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (stream, _) = listener.accept().await?;
        let acceptor = acceptor.clone();
        tokio::spawn(async move {
            if let Ok(tls_stream) = acceptor.accept(stream).await {
                let _ = hyper::server::conn::Http::new()
                    .serve_connection(tls_stream, service_fn(health_handler))
                    .await;
            }
        });
    }
}

// 心跳检测任务，定期清理过期连接
async fn heartbeat_task() -> Result<()> {
    let mut interval = interval(Duration::from_secs(30)); // 每30秒检查一次
    loop {
        interval.tick().await;

        // 清理过期连接
        GLOBAL_STATE.cleanup_expired_connections().await;

        println!("Heartbeat check completed");
    }
}