use crate::appstate::AppState;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::Response,
};
use http::header;
use sha2::{Sha256, Digest};
use tower_governor::key_extractor::KeyExtractor;


// 自定义设备指纹键提取器
#[derive(Clone)]
pub struct DeviceFingerprintKeyExtractor;

impl KeyExtractor for DeviceFingerprintKeyExtractor {
    type Key = String;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, tower_governor::GovernorError> {
        // 从请求中提取设备指纹信息
        let mut hasher = Sha256::new();
        
        // 收集设备指纹信息
        if let Some(user_agent) = req.headers().get("user-agent") {
            hasher.update(user_agent.as_bytes());
        }
        
        if let Some(accept) = req.headers().get("accept") {
            hasher.update(accept.as_bytes());
        }
        
        if let Some(accept_encoding) = req.headers().get("accept-encoding") {
            hasher.update(accept_encoding.as_bytes());
        }
        
        if let Some(accept_language) = req.headers().get("accept-language") {
            hasher.update(accept_language.as_bytes());
        }
        
        if let Some(screen_info) = req.headers().get("x-screen-info") {
            hasher.update(screen_info.as_bytes());
        }
        
        // 添加一些其他可能的指纹信息
        if let Some(canvas_fingerprint) = req.headers().get("x-canvas-fingerprint") {
            hasher.update(canvas_fingerprint.as_bytes());
        }
        
        if let Some(webgl_info) = req.headers().get("x-webgl-info") {
            hasher.update(webgl_info.as_bytes());
        }
        
        // 生成哈希
        let hash = hasher.finalize();
        Ok(format!("{:x}", hash))
    }
}

// 速率限制配置常量
pub const RPC_RATE_LIMIT: u64 = 30;      // RPC 路由: 30 请求/秒
pub const INDEXER_RATE_LIMIT: u64 = 10;  // 索引器路由: 10 请求/秒
pub const FOREX_RATE_LIMIT: u64 = 5;     // 外汇路由: 5 请求/秒
pub const HEALTH_RATE_LIMIT: u64 = 3;    // 健康检查路由: 3 请求/秒

// 速率限制 burst size 常量
pub const RPC_BURST_SIZE: u64 = 30;
pub const INDEXER_BURST_SIZE: u64 = 10;
pub const FOREX_BURST_SIZE: u64 = 5;
pub const HEALTH_BURST_SIZE: u64 = 3;

// 简化的 CORS 头部中间件（只添加必要的头部）
pub async fn add_headers(
    State(_state): State<AppState>,
    req: Request,
    next: axum::middleware::Next,
) -> Result<Response, StatusCode> {
    let mut response = next.run(req).await;

    let headers = response.headers_mut();
    // 允许所有来源，以支持 Android 应用等无域名客户端
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        "*".parse().unwrap(),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        "GET, POST, OPTIONS".parse().unwrap(),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        "Content-Type, Authorization, X-Screen-Info, X-Canvas-Fingerprint, X-WebGL-Info".parse().unwrap(),
    );

    Ok(response)
}

// 简单的健康检查端点
pub async fn health_check() -> &'static str {
    "OK"
}
