use axum::{
    Json,
    extract::{ State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use reqwest::Client;
use tokio::time::{self, Duration};



use crate::appstate::{AppState, ForexData, RawForexData};


// 每小时更新外汇数据
pub async fn update_forex_data(state: AppState) {
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
pub async fn get_forex_data(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.forex_data.read().await.clone())
}

// Forex API 端点（原始数据）
pub async fn get_raw_forex_data(State(state): State<AppState>) -> Response {
    match state.raw_forex_data.read().await.clone() {
        Some(data) => Json(data).into_response(),
        None => Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .body("Forex data not available".into())
            .unwrap(),
    }
}
