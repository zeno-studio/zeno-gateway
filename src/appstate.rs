use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::postgres::PgPool;
use sqlx::postgres::PgPoolOptions;

#[derive(Clone)]
pub struct PostgresDb {
    pub db_url: String,
    pub pool: PgPool,
}

impl PostgresDb {
    pub fn new(db_url: String) -> Self {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_lazy(&db_url)
            .unwrap();
        PostgresDb { db_url, pool }
    }

    pub async fn update_db_url(&mut self, new_url: String) -> Result<()> {
        let new_pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&new_url)
            .await?;

        // 测试写入
        sqlx::query("CREATE TEMPORARY TABLE IF NOT EXISTS health_check (id SERIAL PRIMARY KEY)")
            .execute(&new_pool)
            .await?;
        sqlx::query("INSERT INTO health_check DEFAULT VALUES")
            .execute(&new_pool)
            .await?;
        sqlx::query("DROP TABLE health_check")
            .execute(&new_pool)
            .await?;

        self.db_url = new_url;
        self.pool = new_pool;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub ankr_key: String,
    pub blast_key: String,
    pub openexchange_key: String,
    pub forex_data: Arc<RwLock<ForexData>>,
    pub rpc_endpoints: HashMap<String, String>,
    pub indexer_endpoints: HashMap<String, String>,
    pub metrics: PrometheusMetrics,
    pub client: Client,
    pub postgres_db: PostgresDb,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ForexData {
    pub timestamp: u64,
    pub rates: std::collections::HashMap<String, f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawForexData {
    pub timestamp: u64,
    pub rates: std::collections::HashMap<String, f64>,
}
