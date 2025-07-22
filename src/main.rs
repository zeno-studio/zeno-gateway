use axum::{Router, middleware, routing::get};
use axum_reverse_proxy::ReverseProxy;
use axum_server::tls_rustls::RustlsConfig;
use dotenv::dotenv;
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration;
use tower::ServiceBuilder;
use tower_governor::{
    GovernorLayer, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor,
};

mod appstate;
use appstate::{AppState, ForexData, PrometheusMetrics};

mod endpoint;
use endpoint::{rpc_proxy, setup_ankr_endpoints, setup_blast_endpoints};

mod prometheus;
use prometheus::{metrics_handler, metrics_middleware};

mod forex;
use forex::{get_forex_data, get_raw_forex_data, update_forex_data};

mod filter;
use filter::{add_headers, domain_filter, health_check};

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

    let rpc_rate_limit_layer = ServiceBuilder::new().layer(GovernorLayer {
        config: Arc::new(rpc_governor_conf),
    });

    // Indexer路由：10 RPS
    let indexer_governor_conf = GovernorConfigBuilder::default()
        .per_second(10)
        .burst_size(10)
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .unwrap();

    let indexer_rate_limit_layer = ServiceBuilder::new().layer(GovernorLayer {
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

    let forex_rate_limit_layer = ServiceBuilder::new().layer(GovernorLayer {
        config: Arc::new(forex_governor_conf),
    });

    // health路由：10 RPS
    let health_governor_conf = GovernorConfigBuilder::default()
        .per_second(10)
        .burst_size(10)
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .unwrap();

    let health_rate_limit_layer = ServiceBuilder::new().layer(GovernorLayer {
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
        .layer(middleware::from_fn_with_state(
            state.clone(),
            metrics_middleware,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), add_headers))
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
