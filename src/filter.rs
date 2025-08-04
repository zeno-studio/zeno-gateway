use crate::appstate::AppState;
use axum::{
    extract::{Request, State},
    http::{header, StatusCode, Uri},
    middleware::Next,
    response::Response,
};
use sha2::{Digest, Sha256};
use tower_governor::key_extractor::KeyExtractor;
use tracing::info;

// Device fingerprint key extractor
#[derive(Clone)]
pub struct DeviceFingerprintKeyExtractor;

impl KeyExtractor for DeviceFingerprintKeyExtractor {
    type Key = String;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, tower_governor::GovernorError> {
        let mut hasher = Sha256::new();
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
        if let Some(canvas_fingerprint) = req.headers().get("x-canvas-fingerprint") {
            hasher.update(canvas_fingerprint.as_bytes());
        }
        if let Some(webgl_info) = req.headers().get("x-webgl-info") {
            hasher.update(webgl_info.as_bytes());
        }
        let hash = hasher.finalize();
        Ok(format!("{:x}", hash))
    }
}

// Rate limit constants
pub const RPC_RATE_LIMIT: u64 = 30; // RPC: 30 req/s
pub const INDEXER_RATE_LIMIT: u64 = 10; // Indexer: 10 req/s
pub const FOREX_RATE_LIMIT: u64 = 5; // Forex: 5 req/s
pub const HEALTH_RATE_LIMIT: u64 = 3; // Health: 3 req/s
pub const RPC_BURST_SIZE: u64 = 30;
pub const INDEXER_BURST_SIZE: u64 = 10;
pub const FOREX_BURST_SIZE: u64 = 5;
pub const HEALTH_BURST_SIZE: u64 = 3;

// Simplified CORS middleware
pub async fn add_headers(
    State(_state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
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

// HTTP-to-HTTPS redirect middleware
pub async fn redirect_to_https(req: Request, next: Next) -> Response {
    if req.uri().scheme() == Some(&http::uri::Scheme::HTTP) {
        let mut uri_parts = req.uri().clone().into_parts();
        uri_parts.scheme = Some(http::uri::Scheme::HTTPS);
        uri_parts.authority = Some(http::uri::Authority::from_static("localhost:8443")); // Replace with your domain
        let redirect_uri = Uri::from_parts(uri_parts).unwrap();
        info!(uri = %req.uri(), "Redirecting HTTP to HTTPS: {}", redirect_uri);
        Response::builder()
            .status(StatusCode::PERMANENT_REDIRECT)
            .header(header::LOCATION, redirect_uri.to_string())
            .body(axum::body::Body::empty())
            .unwrap()
    } else {
        next.run(req).await
    }
}

// Simple health check endpoint
pub async fn health_check() -> &'static str {
    "OK"
}
