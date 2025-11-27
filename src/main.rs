// src/main.rs  
use crate::{  
    service::IndexService,   
    auth::{AuthServiceImpl,auth_interceptor},
    error::Result,   
    pb::ankr::ankr_indexer_server::AnkrIndexerServer,  
    pb::auth::auth_service_server::AuthServiceServer,  // 添加这行导入
    state::AppState,  
};   
use hyper::{service::service_fn, Body, Request, Response};  
use rustls::ServerConfig;  
use std::{convert::Infallible, net::SocketAddr, sync::Arc};  
use tokio::net::TcpListener;  
use tokio_rustls::TlsAcceptor;  
use tonic::transport::{Identity, Server, ServerTlsConfig};  
use tower::ServiceBuilder;  
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};  
  
mod ankr;  
mod ankr_types;  
mod db;  
mod error;  
mod pb;  
mod state;  
mod auth;
mod service;
  


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
                    .serve_connection(tls_stream, service_fn(health_handler)).await;  
            }  
        });  
    }  
}  
  
/// 辅助函数：从内存字节构建 Rustls ServerConfig  
fn load_rustls_config(cert: &[u8], key: &[u8]) -> Result<ServerConfig> {  
    let mut cert_reader = std::io::Cursor::new(cert);  
    let certs = rustls_pemfile::certs(&mut cert_reader)  
        .collect::<std::result::Result<Vec<_>, _>>()?;  
  
    let mut key_reader = std::io::Cursor::new(key);  
    // 尝试解析 PKCS8，如果实际是 RSA 或其他格式，可按需添加 fallback  
    let keys: Vec<rustls::pki_types::PrivateKeyDer> = rustls_pemfile::pkcs8_private_keys(&mut key_reader)  
        .collect::<std::result::Result<Vec<_>, _>>()?  
        .into_iter()  
        .map(rustls::pki_types::PrivateKeyDer::from)  
        .collect();  
  
    let private_key = keys.into_iter().next().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "No private keys found"))?;  
  
    let config = ServerConfig::builder()  
        .with_no_client_auth()  
        .with_single_cert(certs, private_key)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Failed to create server config: {}", e)))?;  
          
    Ok(config)  
}

#[tokio::main]  
async fn main() -> Result<()> {  
    tracing_subscriber::fmt::init();  
      
    // 1. 证书读取  
    let cert_pem = tokio::fs::read("./cert.pem").await?;  
    let key_pem = tokio::fs::read("./key.pem").await?;  
  
    // 2. 准备服务实例  
    let state = Arc::new(AppState::new());  
      
    // 业务服务：挂载鉴权拦截器 (check JWT)  
    let indexer = IndexService { state: state.clone() };
    let state_clone = state.clone();
    let indexer_with_auth = AnkrIndexerServer::with_interceptor(indexer, move |req| auth_interceptor(req, &state_clone));
  
    // 登录服务：不挂载拦截器 (公开)  
    let auth = AuthServiceServer::new(AuthServiceImpl { state: state.clone() });
  
    // 3. 配置限流 (基于 IP)  
    // 配置：每秒允许 2 个请求，突发允许 5 个。  
    // 这将应用于所有 gRPC 请求，防止暴力破解登录或刷接口  
    let governor_conf = Box::new(  
        GovernorConfigBuilder::default()  
            .per_second(2)  
            .burst_size(5)  
            .finish()  
            .unwrap(),  
    );  
  
    // 4. 构建 gRPC 路由层  
    let grpc_addr = "0.0.0.0:50051".parse()?;  
    let grpc_identity = Identity::from_pem(&cert_pem, &key_pem);  
      
    // 使用 tower ServiceBuilder 组装全局中间件  
    let layer = ServiceBuilder::new()  
        .layer(GovernorLayer::new(*governor_conf)) // IP 限流  
        .into_inner();  
  
    let grpc_server = Server::builder()  
        .tls_config(ServerTlsConfig::new().identity(grpc_identity))?  
        .layer(layer) // 应用限流层  
        .add_service(auth) // 注册登录服务 (Public)  
        .add_service(indexer_with_auth) // 注册业务服务 (Protected)  
        .serve(grpc_addr);  
  
    // 5. Health Server (不做变动)  
    let http_addr = "0.0.0.0:8443".parse()?;  
    let http_tls_config = Arc::new(load_rustls_config(&cert_pem, &key_pem)?);  
    let http_server = run_health_server(http_addr, http_tls_config);  
  
    println!("gRPC Server listening on {}", grpc_addr);  
      
      tokio::try_join!(  
        async { grpc_server.await.map_err(error::AppError::from) },  
        async { http_server.await.map_err(error::AppError::from) }  
    )?;  
  
    Ok(())  
}  