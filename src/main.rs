use axum::{
    Json, Router,
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum_reverse_proxy::ReverseProxy;
use axum_server::tls_rustls::RustlsConfig;
use dotenv::dotenv;
use http::header;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{self, Duration, Instant};
use tower_governor::{
    governor::GovernorConfigBuilder,
    key_extractor::SmartIpKeyExtractor,
    GovernorLayer,
};
use tower::ServiceBuilder;

// Prometheus 相关导入
use axum_prometheus::PrometheusMetricLayer;
use prometheus::{
    Counter, Histogram, IntCounter, IntGauge, Registry, Encoder, TextEncoder,
    HistogramOpts, Opts, register_counter_with_registry, register_histogram_with_registry,
    register_int_counter_with_registry, register_int_gauge_with_registry,
};
use metrics_exporter_prometheus::PrometheusBuilder;

// 外汇 API 数据结构（精简）
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ForexData {
    timestamp: u64,
    rates: std::collections::HashMap<String, f64>,
}

// 原始外汇 API 响应（用于 /forex/raw）
#[derive(Debug, Clone, Deserialize, Serialize)]
struct RawForexData {
    disclaimer: String,
    license: String,
    timestamp: u64,
    base: String,
    rates: std::collections::HashMap<String, f64>,
}

// Prometheus 指标结构
#[derive(Clone)]
struct PrometheusMetrics {
    registry: Registry,
    http_requests_total: IntCounter,
    http_request_duration: Histogram,
    rpc_requests_total: IntCounter,
    rpc_request_duration: Histogram,
    indexer_requests_total: IntCounter,
    indexer_request_duration: Histogram,
    forex_updates_total: IntCounter,
    active_connections: IntGauge,
    rate_limit_hits: IntCounter,
}

impl PrometheusMetrics {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let registry = Registry::new();

        let http_requests_total = register_int_counter_with_registry!(
            Opts::new("http_requests_total", "Total number of HTTP requests"),
            registry
        )?;

        let http_request_duration = register_histogram_with_registry!(
            HistogramOpts::new("http_request_duration_seconds", "HTTP request duration in seconds"),
            registry
        )?;

        let rpc_requests_total = register_int_counter_with_registry!(
            Opts::new("rpc_requests_total", "Total number of RPC requests"),
            registry
        )?;

        let rpc_request_duration = register_histogram_with_registry!(
            HistogramOpts::new("rpc_request_duration_seconds", "RPC request duration in seconds"),
            registry
        )?;

        let indexer_requests_total = register_int_counter_with_registry!(
            Opts::new("indexer_requests_total", "Total number of indexer requests"),
            registry
        )?;

        let indexer_request_duration = register_histogram_with_registry!(
            HistogramOpts::new("indexer_request_duration_seconds", "Indexer request duration in seconds"),
            registry
        )?;

        let forex_updates_total = register_int_counter_with_registry!(
            Opts::new("forex_updates_total", "Total number of forex data updates"),
            registry
        )?;

        let active_connections = register_int_gauge_with_registry!(
            Opts::new("active_connections", "Number of active connections"),
            registry
        )?;

        let rate_limit_hits = register_int_counter_with_registry!(
            Opts::new("rate_limit_hits_total", "Total number of rate limit hits"),
            registry
        )?;

        Ok(PrometheusMetrics {
            registry,
            http_requests_total,
            http_request_duration,
            rpc_requests_total,
            rpc_request_duration,
            indexer_requests_total,
            indexer_request_duration,
            forex_updates_total,
            active_connections,
            rate_limit_hits,
        })
    }
}

// 应用状态
#[derive(Clone)]
struct AppState {
    ankr_key: String,
    blast_key: String,
    openexchange_key: String,
    forex_data: Arc<RwLock<ForexData>>,
    raw_forex_data: Arc<RwLock<Option<RawForexData>>>, // 存储原始 JSON
    rpc_endpoints: HashMap<String, String>,
    metrics: PrometheusMetrics,
}


// 初始化 Ankr RPC 端点
fn setup_ankr_endpoints(rpc_endpoints: &mut HashMap<String, String>, ankr_key: &str) {
    let chains = vec![
        ("ankr_eth", "eth"),
        ("ankr_bsc", "bsc"),
        ("ankr_arbitrum", "arbitrum"),
        ("ankr_optimism", "optimism"),
        ("ankr_base", "base"),
        ("ankr_polygon", "polygon"),
    ];

    for (endpoint_name, chain) in chains {
        let url = format!("https://rpc.ankr.com/{}/{}", chain, ankr_key);
        rpc_endpoints.insert(endpoint_name.to_string(), url);
    }
}

// 初始化 Blast RPC 端点
fn setup_blast_endpoints(rpc_endpoints: &mut HashMap<String, String>, blast_key: &str) {
    let endpoints = vec![
        (
            "blast_eth",
            format!("https://eth-mainnet.blastapi.io/{}", blast_key),
        ),
        (
            "blast_bsc",
            format!("https://bsc-mainnet.blastapi.io/{}", blast_key),
        ),
        (
            "blast_arbitrum",
            format!("https://arbitrum-one.blastapi.io/{}", blast_key),
        ),
        (
            "blast_optimism",
            format!("https://optimism-mainnet.blastapi.io/{}", blast_key),
        ),
        (
            "blast_base",
            format!("https://base-mainnet.blastapi.io/{}", blast_key),
        ),
        (
            "blast_polygon",
            format!("https://polygon-mainnet.blastapi.io/{}", blast_key),
        ),
    ];

    for (endpoint_name, url) in endpoints {
        rpc_endpoints.insert(endpoint_name.to_string(), url);
    }
}

// 自定义 RPC 代理
async fn rpc_proxy(State(state): State<AppState>, req: Request<Body>) -> Response<Body> {
    let path = req.uri().path().to_string();
    let endpoint_url = match path {
        p if p.starts_with("/rpc/ankr/eth") => state.rpc_endpoints.get("ankr_eth").unwrap(),
        p if p.starts_with("/rpc/ankr/bsc") => state.rpc_endpoints.get("ankr_bsc").unwrap(),
        p if p.starts_with("/rpc/ankr/arbitrum") => {
            state.rpc_endpoints.get("ankr_arbitrum").unwrap()
        }
        p if p.starts_with("/rpc/ankr/optimism") => {
            state.rpc_endpoints.get("ankr_optimism").unwrap()
        }
        p if p.starts_with("/rpc/ankr/base") => state.rpc_endpoints.get("ankr_base").unwrap(),
        p if p.starts_with("/rpc/ankr/polygon") => state.rpc_endpoints.get("ankr_polygon").unwrap(),
        p if p.starts_with("/rpc/blast/eth") => state.rpc_endpoints.get("blast_eth").unwrap(),
        p if p.starts_with("/rpc/blast/bsc") => state.rpc_endpoints.get("blast_bsc").unwrap(),
        p if p.starts_with("/rpc/blast/arbitrum") => {
            state.rpc_endpoints.get("blast_arbitrum").unwrap()
        }
        p if p.starts_with("/rpc/blast/optimism") => {
            state.rpc_endpoints.get("blast_optimism").unwrap()
        }
        p if p.starts_with("/rpc/blast/base") => state.rpc_endpoints.get("blast_base").unwrap(),
        p if p.starts_with("/rpc/blast/polygon") => {
            state.rpc_endpoints.get("blast_polygon").unwrap()
        }
        _ => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not Found"))
                .unwrap();
        }
    };


    // 创建 HTTP 客户端并转发请求
    let client = Client::new();
    let method = req.method().clone();
    let headers = req.headers().clone();
    let body = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .unwrap_or_default();

    let mut request_builder = client.request(method, endpoint_url);

    // 复制请求头
    for (name, value) in headers.iter() {
        if name != "host" && name != "content-length" {
            request_builder = request_builder.header(name, value);
        }
    }

    // 发送请求
    match request_builder.body(body).send().await {
        Ok(response) => {
            let status = response.status();
            let headers = response.headers().clone();
            let body = response.bytes().await.unwrap_or_default();

            let mut response_builder = Response::builder().status(status);

            // 复制响应头
            for (name, value) in headers.iter() {
                if name != "content-length" && name != "transfer-encoding" {
                    response_builder = response_builder.header(name, value);
                }
            }

            response_builder.body(Body::from(body)).unwrap()
        }
        Err(_) => Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Body::from("Bad Gateway"))
            .unwrap(),
    }
}

// 域名过滤中间件
async fn domain_filter(req: Request, next: axum::middleware::Next) -> Result<Response, StatusCode> {
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
async fn add_headers(
    State(_state): State<AppState>,
    req: Request,
    next: axum::middleware::Next,
) -> Response {
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

    response
}

// 简单的健康检查端点
async fn health_check() -> &'static str {
    "OK"
}

// Prometheus 指标端点
async fn metrics_handler(State(state): State<AppState>) -> Response {
    let encoder = TextEncoder::new();
    let metric_families = state.metrics.registry.gather();
    
    match encoder.encode_to_string(&metric_families) {
        Ok(output) => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", encoder.format_type())
            .body(Body::from(output))
            .unwrap(),
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Failed to encode metrics"))
            .unwrap(),
    }
}

// 监控中间件
async fn metrics_middleware(
    State(state): State<AppState>,
    req: Request,
    next: axum::middleware::Next,
) -> Response {
    let start = Instant::now();
    let path = req.uri().path().to_string();
    
    // 增加HTTP请求计数
    state.metrics.http_requests_total.inc();
    
    // 如果是RPC请求，也增加RPC计数
    if path.starts_with("/rpc/") {
        state.metrics.rpc_requests_total.inc();
    }
    
    // 如果是Indexer请求，也增加Indexer计数
    if path.starts_with("/indexer") {
        state.metrics.indexer_requests_total.inc();
    }
    
    let response = next.run(req).await;
    
    // 记录请求持续时间
    let duration = start.elapsed().as_secs_f64();
    state.metrics.http_request_duration.observe(duration);
    
    if path.starts_with("/rpc/") {
        state.metrics.rpc_request_duration.observe(duration);
    }
    
    if path.starts_with("/indexer") {
        state.metrics.indexer_request_duration.observe(duration);
    }
    
    response
}

// 每小时更新外汇数据
async fn update_forex_data(state: AppState) {
    let client = Client::new();
    let url = format!(
        "https://openexchangerates.org/api/latest.json?app_id={}",
        state.openexchange_key
    );
    loop {
        match client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(raw_data) = resp.json::<RawForexData>().await {
                    let forex_data = ForexData {
                        timestamp: raw_data.timestamp,
                        rates: raw_data.rates.clone(),
                    };
                    *state.forex_data.write().await = forex_data;
                    *state.raw_forex_data.write().await = Some(raw_data);
                    
                    // 增加外汇更新计数
                    state.metrics.forex_updates_total.inc();
                    
                    println!("Updated forex data: {:?}", state.forex_data.read().await);
                } else {
                    println!("Failed to parse forex JSON");
                }
            }
            Err(e) => println!("Failed to fetch forex data: {}", e),
        }
        time::sleep(Duration::from_secs(3600)).await; // 每小时更新
    }
}

// Forex API 端点（精简数据）
async fn get_forex_data(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.forex_data.read().await.clone())
}

// Forex API 端点（原始数据）
async fn get_raw_forex_data(State(state): State<AppState>) -> Response {
    match state.raw_forex_data.read().await.clone() {
        Some(data) => Json(data).into_response(),
        None => Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .body("Forex data not available".into())
            .unwrap(),
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok(); // 加载环境变量

    // 获取环境变量
    let ankr_key = env::var("ANKR_API_KEY").unwrap_or_default();
    let blast_key = env::var("BLAST_API_KEY").unwrap_or_default();
    let openexchange_key = env::var("OPENEXCHANGE_KEY").unwrap_or_default();

    // 初始化 RPC 端点
    let mut rpc_endpoints = HashMap::new();
    setup_ankr_endpoints(&mut rpc_endpoints, &ankr_key);
    setup_blast_endpoints(&mut rpc_endpoints, &blast_key);

    // 初始化 Prometheus 指标
    let metrics = PrometheusMetrics::new().expect("Failed to create Prometheus metrics");

    // 初始化应用状态
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
        metrics,
    };

    // 定时更新外汇数据
    tokio::spawn(update_forex_data(state.clone()));

    let indexer_url = format!("https://rpc.ankr.com/multichain/{}", state.ankr_key);

    // 配置不同的速率限制
    
    // RPC路由：30 RPS
    let rpc_governor_conf = GovernorConfigBuilder::default()
        .per_second(30)
        .burst_size(30)
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .unwrap();
    
    let rpc_rate_limit_layer = ServiceBuilder::new()
        .layer(GovernorLayer {
            config: Arc::new(rpc_governor_conf),
        });

    // Indexer路由：10 RPS
    let indexer_governor_conf = GovernorConfigBuilder::default()
        .per_second(10)
        .burst_size(10)
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .unwrap();
    
    let indexer_rate_limit_layer = ServiceBuilder::new()
        .layer(GovernorLayer {
            config: Arc::new(indexer_governor_conf),
        });

    // Forex路由：1次/分钟 (1/60秒)
    let forex_governor_conf = GovernorConfigBuilder::default()
        .per_second(1)
        .burst_size(1)
        .period(Duration::from_secs(60)) // 60秒窗口期
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .unwrap();

   
    let forex_rate_limit_layer = ServiceBuilder::new()
        .layer(GovernorLayer {
            config: Arc::new(forex_governor_conf),
        });

     // health路由：10 RPS
    let health_governor_conf = GovernorConfigBuilder::default()
        .per_second(10)
        .burst_size(10)
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .unwrap();

    
    let health_rate_limit_layer = ServiceBuilder::new()
        .layer(GovernorLayer {
            config: Arc::new(health_governor_conf),
        });
    


    // RPC路由（30 RPS）
    let rpc_routes = Router::new()
        .route("/rpc/ankr/{*path}", get(rpc_proxy).post(rpc_proxy))
        .route("/rpc/blast/{*path}", get(rpc_proxy).post(rpc_proxy))
        .with_state(state.clone())
        .layer(rpc_rate_limit_layer);

    // Indexer路由（10 RPS）
    let indexer_routes = Router::new()
        .merge(ReverseProxy::new("/indexer", &indexer_url))
        .with_state(state.clone())
        .layer(indexer_rate_limit_layer);

    // Forex路由（1次/分钟）
    let forex_routes = Router::new()
        .route("/forex", get(get_forex_data))
        .route("/forex/raw", get(get_raw_forex_data))
        .with_state(state.clone())
        .layer(forex_rate_limit_layer);

    // health检查和metrics
    let health_routes = Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .with_state(state.clone())
        .layer(health_rate_limit_layer);




    // 合并所有路由
    let app = Router::new()
        .merge(rpc_routes)
        .merge(indexer_routes)
        .merge(forex_routes)
        .merge(health_routes)
        .layer(middleware::from_fn_with_state(state.clone(), metrics_middleware))
        .layer(middleware::from_fn_with_state(state, add_headers))
        .layer(middleware::from_fn(domain_filter));

    // TLS 证书路径配置（支持多种配置方式）
    let cert_path_str = env::var("TLS_CERT_PATH").unwrap_or_else(|_| {
        // 按优先级检查多个可能的证书路径
        let possible_paths = [
            "/etc/ssl/certs/zeno-gateway.crt",  // 系统级证书路径
            "/opt/zeno-gateway/certs/cert.pem", // 应用专用目录
            "./certs/cert.pem",                 // 项目子目录
            "./cert.pem",                       // 项目根目录（当前默认）
        ];
        
        for path in &possible_paths {
            if Path::new(path).exists() {
                return path.to_string();
            }
        }
        
        "cert.pem".to_string() // 默认回退到项目根目录
    });
    
    let key_path_str = env::var("TLS_KEY_PATH").unwrap_or_else(|_| {
        // 按优先级检查多个可能的私钥路径
        let possible_paths = [
            "/etc/ssl/private/zeno-gateway.key", // 系统级私钥路径
            "/opt/zeno-gateway/certs/key.pem",   // 应用专用目录
            "./certs/key.pem",                   // 项目子目录
            "./key.pem",                         // 项目根目录（当前默认）
        ];
        
        for path in &possible_paths {
            if Path::new(path).exists() {
                return path.to_string();
            }
        }
        
        "key.pem".to_string() // 默认回退到项目根目录
    });

    let cert_path = Path::new(&cert_path_str);
    let key_path = Path::new(&key_path_str);

    if cert_path.exists() && key_path.exists() {
        // 启动 HTTPS 服务器
        let tls_config = RustlsConfig::from_pem_file(cert_path, key_path)
            .await
            .expect("Failed to load TLS certificates");

        println!("TLS certificates found:");
        println!("  Certificate: {}", cert_path.display());
        println!("  Private Key: {}", key_path.display());
        println!("Server running on https://0.0.0.0:8443");
        
        axum_server::bind_rustls("0.0.0.0:8443".parse().unwrap(), tls_config)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap();
    } else {
        // 如果没有证书文件，启动 HTTP 服务器（用于开发/测试）
        println!("TLS certificates not found. Checked paths:");
        println!("  Certificate: {}", cert_path.display());
        println!("  Private Key: {}", key_path.display());
        println!();
        println!("Starting HTTP server for development/testing...");
        println!("For production HTTPS, provide certificates using one of these methods:");
        println!("  1. Environment variables: TLS_CERT_PATH and TLS_KEY_PATH");
        println!("  2. Place files in: /etc/ssl/certs/ and /etc/ssl/private/");
        println!("  3. Place files in: /opt/zeno-gateway/certs/");
        println!("  4. Place files in: ./certs/ (project subdirectory)");
        println!("  5. Place files in: ./ (project root directory)");
        println!();
        
        let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
        println!("Server running on http://0.0.0.0:3000");
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    }

}
