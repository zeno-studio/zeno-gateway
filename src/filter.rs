use crate::appstate::AppState;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::Response,
    body::Body,
};
use http::header;


// 域名过滤中间件
pub async fn domain_filter(req: Request, next: axum::middleware::Next) -> Result<Response, StatusCode> {
    // 检查 Host 头部
    if let Some(host) = req.headers().get("host") {
        if let Ok(host_str) = host.to_str() {
            // 允许的域名列表
            let allowed_domains = ["zeno.qw", "localhost", "127.0.0.1"];

            // 检查是否是允许的域名（支持端口号）
            let domain = host_str.split(':').next().unwrap_or(host_str);
            if allowed_domains.iter().any(|&allowed| domain == allowed) {
                Ok(next.run(req).await)
            } else {
                println!("Blocked request from domain: {}", host_str);
                Err(StatusCode::FORBIDDEN)
            }
        } else {
            Err(StatusCode::BAD_REQUEST)
        }
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

// 添加 CORS 头部的中间件
pub async fn add_headers(
    State(_state): State<AppState>,
    req: Request,
    next: axum::middleware::Next,
) -> Result<Response, StatusCode> {
    let mut response = next.run(req).await;

    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        "https://zeno.qw".parse().unwrap(),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        "GET, POST, OPTIONS".parse().unwrap(),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        "Content-Type, Authorization".parse().unwrap(),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
        "true".parse().unwrap(),
    );

    Ok(response)
}

// 简单的健康检查端点
pub async fn health_check() -> &'static str {
    "OK"
}

// Middleware to restrict /metrics endpoint
pub async fn restrict_metrics(req: Request<Body>, next: axum::middleware::Next) -> Response {
    if req.uri().path() == "/metrics" {
        let client_ip = req
            .extensions()
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
            .map(|info| info.0.ip());
        if client_ip != Some(std::net::IpAddr::from([127, 0, 0, 1])) {
            return Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body(Body::from("Access to metrics endpoint forbidden"))
                .unwrap();
        }
    }
    next.run(req).await
}

