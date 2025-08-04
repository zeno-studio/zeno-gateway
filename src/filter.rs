use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
    body::Body,
};
use std::sync::Arc;
use prometheus::CounterVec;
use std::time::{SystemTime, UNIX_EPOCH};
use dashmap::DashMap;

use crate::appstate::AppState;


pub async fn add_headers(
    State(_state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let mut response = next.run(req).await;

    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        "*".parse().unwrap(), // 允许任意来源
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        "GET, POST, OPTIONS".parse().unwrap(),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        "Content-Type, Authorization, X-Device-Fingerprint-Hash".parse().unwrap(), // 添加设备指纹头
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
        "true".parse().unwrap(),
    );

    Ok(response)
}

pub async fn health_check() -> &'static str {
    "OK"
}

// 限流状态存储
#[derive(Clone)]
pub struct RateLimiter {
    counters: Arc<DashMap<String, (u64, u64)>>, // (timestamp, count)
    metrics: Arc<CounterVec>, // 限流拒绝计数
}

impl RateLimiter {
    pub fn new(metrics: Arc<CounterVec>) -> Self {
        RateLimiter {
            counters: Arc::new(DashMap::new()),
            metrics,
        }
    }

    pub async fn check_rate_limit(&self, hash: &str, path: &str) -> Result<(), StatusCode> {
        // 根据路由设置不同限流阈值
        let (limit, window_secs) = match path {
            p if p.starts_with("/rpc") => (100, 60), // 每分钟 100 次
            p if p.starts_with("/indexer") => (50, 60), // 每分钟 50 次
            p if p.starts_with("/forex") => (20, 60), // 每分钟 20 次
            p if p == "/metrics" => (10, 60), // 每分钟 10 次，仅内部
            p if p == "/health" => (50, 60), // 每分钟 50 次
            _ => (50, 60), // 默认
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut entry = self.counters.entry(hash.to_string()).or_insert((now, 0));
        let (last_time, count) = *entry;

        // 重置计数器如果时间窗口已过
        if now - last_time >= window_secs {
            *entry = (now, 1);
            Ok(())
        } else if count < limit {
            *entry = (last_time, count + 1);
            Ok(())
        } else {
            self.metrics.with_label_values(&[path, hash]).inc();
            Err(StatusCode::TOO_MANY_REQUESTS)
        }
    }
}

pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let hash = req
        .headers()
        .get("X-Device-Fingerprint-Hash")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string()); // 默认值

    let path = req.uri().path().to_string();
    let rate_limiter = RateLimiter {
        counters: Arc::new(DashMap::new()), // 在生产环境中应使用 Redis
        metrics: Arc::new(state.metrics.rate_limit_exceeded_total.clone()),
    };

    // 对 /metrics 端点额外限制
    if path == "/metrics" && hash != "internal-service-hash" { // 假设内部服务哈希
        return Err(StatusCode::FORBIDDEN);
    }

    rate_limiter.check_rate_limit(&hash, &path).await?;
    Ok(next.run(req).await)
}
