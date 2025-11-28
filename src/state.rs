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
}

impl AppState {
    pub fn new() -> Self {
        dotenvy::dotenv().ok();
        let ankr_key = env::var("ANKR_API_KEY").unwrap_or_default();
        let db_url = env::var("DATABASE_URL").unwrap_or_default();
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
            db         // 直接使用 String
        }
    }
}

pub struct IndexService {
    pub state: Arc<AppState>,
}