use axum::{Router, middleware, routing::get};
use axum_reverse_proxy::ReverseProxy;
use dotenv::dotenv;
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration;
use tower::ServiceBuilder;
use tower_governor::{
    GovernorLayer, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor,
};
use rustls_acme::caches::DirCache;

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

    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // 获取环境变量
    let ankr_key = env::var("ANKR_API_KEY").unwrap_or_default();
    let blast_key = env::var("BLAST_API_KEY").unwrap_or_default();
    let openexchange_key = env::var("OPENEXCHANGE_KEY").unwrap_or_default();

    // 获取ACME配置
    let acme_contact = env::var("ACME_CONTACT")
        .unwrap_or_else(|_| "mailto:admin@example.com".to_string());
    let acme_directory = env::var("ACME_DIRECTORY")
        .unwrap_or_else(|_| "https://acme-v02.api.letsencrypt.org/directory".to_string());
    let domain = env::var("DOMAIN").unwrap_or_else(|_| "localhost".to_string());
    let acme_cache_dir = env::var("ACME_CACHE_DIR")
        .unwrap_or_else(|_| "./acme-cache".to_string());

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

    // 检查是否启用HTTPS
    let enable_https = env::var("ENABLE_HTTPS")
        .unwrap_or_else(|_| "true".to_string())
        .parse::<bool>()
        .unwrap_or(true);

    if enable_https {
        // 获取ACME缓存目录
        let acme_cache_dir = env::var("ACME_CACHE_DIR")
            .unwrap_or_else(|_| "./acme-cache".to_string());
        
        // 创建ACME缓存目录
        tokio::fs::create_dir_all(&acme_cache_dir).await
            .expect("Failed to create ACME cache directory");

        // 使用ACME自动证书管理
        tracing::info!("Starting HTTPS server with ACME certificate management");
        tracing::info!("Domain: {}", domain);
        tracing::info!("ACME Directory: {}", acme_directory);
        tracing::info!("Contact: {}", acme_contact);
        tracing::info!("Cache Directory: {}", acme_cache_dir);

        // 配置ACME
        let acme_config = rustls_acme::AcmeConfig::new(vec![domain.clone()])
            .contact(vec![acme_contact])
            .cache(DirCache::new(acme_cache_dir))
            .directory(acme_directory)
            .state();

        // 创建ACME接受器
        let acceptor = acme_config.axum_acceptor(acme_config.default_rustls_config());

        // 启动HTTPS服务器
        let addr = SocketAddr::from(([0, 0, 0, 0], 8443));
        tracing::info!("HTTPS server starting on {}", addr);

        // 使用axum_server::bind_rustls_acme启动服务器
        let server = axum_server::bind(addr)
            .acceptor(acceptor)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>());

        // 运行服务器
        if let Err(e) = server.await {
            tracing::error!("Server error: {}", e);
        }
    } else {
        // 启动HTTP服务器（用于开发/测试）
        tracing::warn!("HTTPS disabled, starting HTTP server for development/testing...");
        
        let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
        let listener = tokio::net::TcpListener::bind(addr).await
            .expect("Failed to bind to address");
        
        tracing::info!("HTTP server starting on http://{}", addr);
        
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("Server failed");
    }
}
