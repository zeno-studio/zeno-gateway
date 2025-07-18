use acme2::{Account, AuthorizationStatus, ChallengeType, Csr, Directory, DirectoryUrl, OrderStatus};
use axum::{
    extract::{Request, State},
    middleware,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use axum_reverse_proxy::ReverseProxy;
use axum_server::tls_rustls::RustlsConfig;
use http::header;
use rand::seq::SliceRandom;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{self, Duration};
use tower_governor::{governor::Governor, GovernorConfigBuilder};
use std::net::{IpAddr, Ipv4Addr};

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

// 应用状态
#[derive(Clone)]
struct AppState {
    ankr_key: String,
    blast_key: String,
    goldrush_key: String,
    openexchange_key: String,
    forex_data: Arc<RwLock<ForexData>>,
    raw_forex_data: Arc<RwLock<Option<RawForexData>>>, // 存储原始 JSON
    rpc_endpoints: std::collections::HashMap<String, Vec<String>>,
}

// 添加认证头
async fn add_headers(mut req: Request, next: middleware::Next, State(state): State<AppState>) -> Response {
    let path = req.uri().path();
    match path {
        p if p.starts_with("/rpc/ankr") => {
            req.headers_mut().insert(header::AUTHORIZATION, format!("Bearer {}", state.ankr_key).parse().unwrap());
            req.headers_mut().insert(header::HOST, "rpc.ankr.com".parse().unwrap());
        }
        p if p.starts_with("/rpc/blast") => {
            req.headers_mut().insert(header::AUTHORIZATION, format!("Bearer {}", state.blast_key).parse().unwrap());
            req.headers_mut().insert(header::HOST, "blastapi.io".parse().unwrap());
        }
        p if p.starts_with("/indexer") => {
            req.headers_mut().insert("X-Api-Key", state.goldrush_key.parse().unwrap());
            req.headers_mut().insert(header::HOST, "api.goldrush.io".parse().unwrap());
        }
        _ => {}
    }
    next.run(req).await
}

// ACME HTTP-01 挑战端点
async fn acme_challenge(req: Request) -> Response {
    let path = req.uri().path();
    if path.starts_with("/.well-known/acme-challenge/") {
        let token = path.strip_prefix("/.well-known/acme-challenge/").unwrap();
        if let Ok(content) = fs::read_to_string(format!("acme-challenges/{}", token)) {
            return Response::new(content.into());
        }
    }
    Response::builder().status(404).body("Not Found".into()).unwrap()
}

// 获取 Let’s Encrypt 证书
async fn obtain_certificate(domain: &str, contact_email: &str) -> Result<(Vec<u8>, Vec<u8>), acme2::Error> {
    let dir = Directory::from_url(DirectoryUrl::LetsEncrypt)?;
    let account = Account::create(&dir, contact_email, None).await?;
    let mut order = account.order(&[domain]).await?;

    for auth in order.authorizations().await? {
        if let Some(challenge) = auth.get_challenge(ChallengeType::Http01) {
            let token = challenge.token();
            let content = challenge.key_authorization(&account)?;
            fs::create_dir_all("acme-challenges")?;
            fs::write(format!("acme-challenges/{}", token), content)?;
            challenge.validate().await?;
            while auth.status != AuthorizationStatus::Valid {
                time::sleep(Duration::from_secs(5)).await;
                auth.refresh().await?;
            }
        }
    }

    let key_pair = acme2::gen_rsa_private_key(2048)?;
    let order = order.finalize(Csr::Automatic(key_pair.clone())).await?;
    while order.status != OrderStatus::Valid {
        time::sleep(Duration::from_secs(5)).await;
        order.refresh().await?;
    }

    let cert = order.certificate().await?.unwrap();
    Ok((cert, key_pair))
}

// 每小时更新外汇数据
async fn update_forex_data(state: AppState) {
    let client = Client::new();
    let url = format!("https://openexchangerates.org/api/latest.json?app_id={}", state.openexchange_key);
    loop {
        match client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(raw_data) = resp.json::<RawForexData>().await {
                    let forex_data = ForexData {
                        timestamp: raw_data.timestamp,
                        rates: raw_data.rates,
                    };
                    *state.forex_data.write().await = forex_data;
                    *state.raw_forex_data.write().await = Some(raw_data);
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
async fn get_raw_forex_data(State(state): State<AppState>) -> impl IntoResponse {
    match state.raw_forex_data.read().await.clone() {
        Some(data) => Json(data),
        None => Response::builder().status(503).body("Forex data not available".into()).unwrap(),
    }
}

// 自定义 RPC 代理（随机选择端点）
async fn rpc_proxy(req: Request, State(state): State<AppState>) -> Response {
    let path = req.uri().path().to_string();
    let endpoints = match path {
        p if p.starts_with("/rpc/ankr") => state.rpc_endpoints.get("ankr").unwrap(),
        p if p.starts_with("/rpc/blast") => state.rpc_endpoints.get("blast").unwrap(),
        _ => return Response::builder().status(404).body("Not Found".into()).unwrap(),
    };
    let endpoint = endpoints.choose(&mut rand::thread_rng()).unwrap();
    ReverseProxy::new("", endpoint).handle(req).await
}

#[tokio::main]
async fn main() {
    // 初始化状态
    let mut rpc_endpoints = std::collections::HashMap::new();
    rpc_endpoints.insert("ankr".to_string(), vec!["https://rpc.ankr.com".to_string(), "https://backup.ankr.com".to_string()]);
    rpc_endpoints.insert("blast".to_string(), vec!["https://blastapi.io".to_string(), "https://backup.blastapi.io".to_string()]);

    let state = AppState {
        ankr_key: env!("ANKR_API_KEY").to_string(),
        blast_key: env!("BLAST_API_KEY").to_string(),
        goldrush_key: env!("GOLDRUSH_API_KEY").to_string(),
        openexchange_key: env!("OPENEXCHANGE_KEY").to_string(),
        forex_data: Arc::new(RwLock::new(ForexData {
            timestamp: 0,
            rates: std::collections::HashMap::new(),
        })),
        raw_forex_data: Arc::new(RwLock::new(None)),
        rpc_endpoints,
    };

    // 获取初始证书
    let domain = "api.example.com"; // 替换为你的域名
    let (cert, key) = obtain_certificate(domain, "admin@example.com").await.unwrap();
    fs::write("cert.pem", &cert).unwrap();
    fs::write("key.pem", &key).unwrap();

    // TLS 配置
    let tls_config = Arc::new(RustlsConfig::from_pem_file(Path::new("cert.pem"), Path::new("key.pem")).await.unwrap());

    // 定时续签证书（每 30 天）
    let tls_config_clone = Arc::clone(&tls_config);
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(30 * 24 * 3600)); // 30 天
        loop {
            interval.tick().await;
            if let Ok((cert, key)) = obtain_certificate("api.example.com", "admin@example.com").await {
                fs::write("cert.pem", &cert).unwrap();
                fs::write("key.pem", &key).unwrap();
                tls_config_clone.reload_from_pem_file(Path::new("cert.pem"), Path::new("key.pem")).await.unwrap();
                println!("Certificate renewed");
            } else {
                println!("Certificate renewal failed");
            }
        }
    });

    // 定时更新外汇数据
    tokio::spawn(update_forex_data(state.clone()));

    // 限流配置
    let governor_conf = GovernorConfigBuilder::default()
        .per_second(1)
        .burst_size(10)
        .finish()
        .unwrap();
    let governor = Governor::new(&governor_conf, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));

    // 路由
    let app: Router = Router::new()
        .route("/rpc/ankr/*path", get(rpc_proxy).post(rpc_proxy))
        .route("/rpc/blast/*path", get(rpc_proxy).post(rpc_proxy))
        .merge(ReverseProxy::new("/indexer", "https://api.goldrush.io"))
        .route("/forex", get(get_forex_data))
        .route("/forex/raw", get(get_raw_forex_data))
        .route("/.well-known/acme-challenge/*path", get(acme_challenge))
        .layer(middleware::from_fn_with_state(state, add_headers))
        .layer(governor);

    // 启动 HTTPS 服务器
    axum_server::bind_rustls("0.0.0.0:443".parse().unwrap(), tls_config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}