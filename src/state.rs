use crate::db::PostgresDb;
use reqwest::Client;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

#[derive(Clone, Debug)]
pub struct AppState {
    pub ankr_key: String,      // 改为 String 类型
    pub client: Arc<Client>,
    pub db: PostgresDb,
    pub master_key: String,    // 改为 String 类型
    pub token_expires_in: usize,
    pub jwt_secret: String,    // 改为 String 类型
    
}

impl AppState {
    pub fn new() -> Self {
        dotenvy::dotenv().ok();
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        let ankr_key = env::var("ANKR_API_KEY").unwrap_or_default();
        let db_url = env::var("DATABASE_URL").unwrap_or_default();
        let master_key = env::var("MASTER_API_KEY").unwrap_or_default();
        let token_expires_in = env::var("TOKEN_EXPIRES_IN").unwrap_or_default();
        let jwt_secret = env::var("JWT_SECRET").unwrap_or_default();
        let db = PostgresDb::new(db_url);
        let client = Client::builder()
            .use_rustls_tls()
            .pool_max_idle_per_host(10)
            .http2_keep_alive_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(10))
            .gzip(true)
            .brotli(true)
            .build()
            .expect("Failed to build reqwest client");
        info!("Built reqwest client with rustls TLS");   
        AppState {
            ankr_key,              // 直接使用 String
            client: Arc::new(client),
            db,
            master_key,            // 直接使用 String
            token_expires_in: token_expires_in.parse().unwrap_or(900),
            jwt_secret,            // 直接使用 String
        }
    }
}