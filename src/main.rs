use axum::{Router, middleware, routing::get};
use axum_server::tls_rustls::RustlsConfig;
use dotenv::dotenv;
use reqwest::Client;
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
use endpoint::{
    indexer_proxy, rpc_proxy, setup_ankr_endpoints, setup_blast_endpoints, setup_indexer_endpoints,
};

mod prometheus;
use prometheus::{metrics_handler, metrics_middleware};

mod forex;
use forex::{get_forex_data, get_raw_forex_data, update_forex_data};

mod filter;
use filter::{add_headers, domain_filter, health_check, restrict_metrics};

#[tokio::main]
async fn main() {
    dotenv().ok(); // 加载环境变量

    // 获取环境变量
    let ankr_key = env::var("ANKR_API_KEY").unwrap_or_default();
    let blast_key = env::var("BLAST_API_KEY").unwrap_or_default();
    let openexchange_key = env::var("OPENEXCHANGE_KEY").unwrap_or_default();

    // 初始化 RPC 端点
    let mut rpc_endpoints = HashMap::new();
    let mut indexer_endpoints = HashMap::new();
    setup_ankr_endpoints(&mut rpc_endpoints, &ankr_key);
    setup_blast_endpoints(&mut rpc_endpoints, &blast_key);
    setup_indexer_endpoints(&mut indexer_endpoints, &ankr_key);

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
        indexer_endpoints, // 初始化 indexer_endpoints
        metrics: PrometheusMetrics::new(),
        client: Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build reqwest client"),
    };

    // 定时更新外汇数据
    tokio::spawn(update_forex_data(state.clone()));

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

    // Forex路由：1rps
    let forex_governor_conf = GovernorConfigBuilder::default()
        .per_second(5)
        .burst_size(5)
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .unwrap();
    let forex_rate_limit_layer = ServiceBuilder::new().layer(GovernorLayer {
        config: Arc::new(forex_governor_conf),
    });

    // health路由：10 RPS
    let health_governor_conf = GovernorConfigBuilder::default()
        .per_second(3)
        .burst_size(3)
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .unwrap();

    let health_rate_limit_layer = ServiceBuilder::new().layer(GovernorLayer {
        config: Arc::new(health_governor_conf),
    });

    // RPC路由（30 RPS）
    let rpc_routes = Router::new()
        .route("/rpc/{provider}/{chain}", get(rpc_proxy).post(rpc_proxy))
        .with_state(state.clone())
        .layer(rpc_rate_limit_layer);

    // Indexer路由（10 RPS）
    let indexer_routes = Router::new()
        .route(
            "/indexer/{provider}",
            get(indexer_proxy).post(indexer_proxy),
        )
        .with_state(state.clone())
        .layer(indexer_rate_limit_layer);

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

    let cert_path_str = env::var("TLS_CERT_PATH").unwrap_or("./cert.pem".to_string());
    let key_path_str = env::var("TLS_KEY_PATH").unwrap_or("./key.pem".to_string());

    let cert_path = Path::new(&cert_path_str);
    let key_path = Path::new(&key_path_str);

    if cert_path.is_file() && key_path.is_file() {
        match std::fs::metadata(cert_path).and_then(|_| std::fs::metadata(key_path)) {
            Ok(_) => match RustlsConfig::from_pem_file(cert_path, key_path).await {
                Ok(tls_config) => {
                    println!("Current user: {:?}", std::env::var("USER"));
                    println!("TLS certificates loaded successfully:");
                    println!("  Certificate: {}", cert_path.display());
                    println!("  Private Key: {}", key_path.display());
                    println!("Server running on https://0.0.0.0:8443");

                    axum_server::bind_rustls("0.0.0.0:8443".parse().unwrap(), tls_config)
                        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
                        .await
                        .unwrap();
                }
                Err(e) => {
                    println!("Failed to load TLS certificates: {}", e);
                    println!("Falling back to HTTP server on http://0.0.0.0:3000...");
                    start_http_server(app).await;
                }
            },
            Err(e) => {
                println!("Cannot access certificate or key files: {}", e);
                println!("  Certificate: {}", cert_path.display());
                println!("  Private Key: {}", key_path.display());
                println!("Falling back to HTTP server on http://0.0.0.0:3000...");
                start_http_server(app).await;
            }
        }
    } else {
        println!("Certificate or key file not found:");
        if !cert_path.is_file() {
            println!("  Certificate: {} (not a file)", cert_path.display());
        }
        if !key_path.is_file() {
            println!("  Private Key: {} (not a file)", key_path.display());
        }
        println!("Falling back to HTTP server on http://0.0.0.0:3000...");
        start_http_server(app).await;
    }

    async fn start_http_server(app: Router) {
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
