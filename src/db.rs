use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct PostgresDb {
    pub db_url: String,
    pub pool: PgPool,
}

impl PostgresDb {
    pub fn new(db_url: String) -> Self {
        // 如果数据库URL为空，则使用默认值或跳过初始化
        let pool = if db_url.is_empty() {
            // 创建一个空的连接池占位符
            PgPoolOptions::new()
                .max_connections(1)
                .acquire_timeout(Duration::from_secs(1))
                .connect_lazy("postgresql://placeholder@localhost/placeholder")
                .expect("Failed to create placeholder pool")
        } else {
            PgPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(Duration::from_secs(3))
                .connect_lazy(&db_url)
                .expect("Failed to create pool")
        };
        
        PostgresDb {
            db_url,
            pool,
        }
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