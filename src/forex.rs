use axum::{extract::State, response::IntoResponse, Json, http::StatusCode};
use serde_json::Value;
use anyhow::Result;
use crate::config::Config;

pub async fn get_forex(State(config): State<Config>) -> Result<impl IntoResponse, (StatusCode, String)> {
    let record: Option<(Value,)> = sqlx::query_as("SELECT data FROM forex_rates LIMIT 1")
        .fetch_optional(&config.postgres_db.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match record {
        Some((data,)) => Ok(Json(data)),
        None => Err((StatusCode::NOT_FOUND, "No forex data found".to_string())),
    }
}
