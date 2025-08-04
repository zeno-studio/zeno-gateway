use crate::api::{LatestForexRequest, LatestForexResponse, forex_service_server::ForexService};
use crate::appstate::AppState;
use tonic::{Request, Response, Status};


use tokio::time::{self, Duration};

use crate::appstate::{ForexData, RawForexData};

pub async fn update_latest_forex_data(state: AppState) {
    let client = &state.client;
    let url = format!(
        "https://openexchangerates.org/api/latest.json?app_id={}",
        state.openexchange_key
    );
    loop {
        match client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(raw_data) = resp.json::<RawForexData>().await {
                    let latest_forex_data = ForexData {
                        timestamp: raw_data.timestamp,
                        rates: raw_data.rates.clone(),
                    };
                    *state.latest_forex_data.write().await = latest_forex_data;
                } else {
                    println!("Failed to parse forex JSON");
                }
            }
            Err(e) => println!("Failed to fetch forex data: {}", e),
        }
        time::sleep(Duration::from_secs(3600)).await; // 每小时更新
    }
}

#[derive(Debug, Clone)]
pub struct GrpcService {
    pub state: AppState,
}

#[tonic::async_trait]
impl ForexService for GrpcService {
    async fn get_latest_forex_data(
        &self,
        _request: Request<LatestForexRequest>,
    ) -> Result<Response<LatestForexResponse>, Status> {
        let data = self.state.latest_forex_data.read().await;
        Ok(Response::new(LatestForexResponse {
            timestamp: data.timestamp,
            rates: data.rates.clone(),
        }))
    }
}
