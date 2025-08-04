use crate::api::{
    AnkrIndexerRequest, AnkrIndexerResponse, AnkrNftBalancesResponse, AnkrTokenBalancesResponse,
    AnkrTransactionsResponse, ankr_indexer_service_server::AnkrIndexerService,
};
use crate::appstate::AppState;
use crate::common::{send_indexer_request, send_jsonrpc_request};
use serde_json::{Value, json};
use tonic::{Request, Response, Status};

// JSON 到 Protobuf 转换函数
fn convert_nft_result(result: &Value) -> Result<AnkrNftBalancesResponse, Status> {
    let assets = result["assets"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|asset| {
            let asset_obj = asset.as_object()?;
            let traits = asset_obj["traits"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|trait_| {
                    let trait_obj = trait_.as_object()?;
                    Some(crate::api::ankr_nft_balances_response::asset::Trait {
                        bunny_id: trait_obj["bunny_id"].as_str().unwrap_or("").to_string(),
                        count: trait_obj["count"].as_i64().unwrap_or(0) as i32,
                        display_type: trait_obj["display_type"].as_str().unwrap_or("").to_string(),
                        frequency: trait_obj["frequency"].as_str().unwrap_or("").to_string(),
                        mp_score: trait_obj["mp_score"].as_str().unwrap_or("").to_string(),
                        rarity: trait_obj["rarity"].as_str().unwrap_or("").to_string(),
                        trait_type: trait_obj["trait_type"].as_str().unwrap_or("").to_string(),
                        value: trait_obj["value"].as_str().unwrap_or("").to_string(),
                    })
                })
                .collect::<Vec<_>>();

            Some(crate::api::ankr_nft_balances_response::Asset {
                blockchain: asset_obj["blockchain"].as_str().unwrap_or("").to_string(),
                collection_name: asset_obj["collectionName"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                contract_address: asset_obj["contractAddress"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                contract_type: asset_obj["contractType"].as_i64().unwrap_or(0) as i32,
                image_url: asset_obj["imageUrl"].as_str().unwrap_or("").to_string(),
                name: asset_obj["name"].as_str().unwrap_or("").to_string(),
                quantity: asset_obj["quantity"].as_str().unwrap_or("").to_string(),
                symbol: asset_obj["symbol"].as_str().unwrap_or("").to_string(),
                token_id: asset_obj["tokenId"].as_str().unwrap_or("").to_string(),
                token_url: asset_obj["tokenUrl"].as_str().unwrap_or("").to_string(),
                traits,
            })
        })
        .collect::<Vec<_>>();

    Ok(AnkrNftBalancesResponse {
        assets,
        next_page_token: result["nextPageToken"].as_str().unwrap_or("").to_string(),
        owner: result["owner"].as_str().unwrap_or("").to_string(),
        error: "".to_string(),
    })
}

fn convert_token_result(result: &Value) -> Result<AnkrTokenBalancesResponse, Status> {
    let assets = result["assets"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|asset| {
            let asset_obj = asset.as_object()?;
            Some(crate::api::ankr_token_balances_response::Asset {
                balance: asset_obj["balance"].as_str().unwrap_or("").to_string(),
                balance_raw_integer: asset_obj["balanceRawInteger"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                balance_usd: asset_obj["balanceUsd"].as_str().unwrap_or("").to_string(),
                blockchain: asset_obj["blockchain"].as_str().unwrap_or("").to_string(),
                contract_address: asset_obj["contractAddress"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                holder_address: asset_obj["holderAddress"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                thumbnail: asset_obj["thumbnail"].as_str().unwrap_or("").to_string(),
                token_decimals: asset_obj["tokenDecimals"].as_i64().unwrap_or(0) as i32,
                token_name: asset_obj["tokenName"].as_str().unwrap_or("").to_string(),
                token_price: asset_obj["tokenPrice"].as_str().unwrap_or("").to_string(),
                token_symbol: asset_obj["tokenSymbol"].as_str().unwrap_or("").to_string(),
                token_type: asset_obj["tokenType"].as_str().unwrap_or("").to_string(),
            })
        })
        .collect::<Vec<_>>();

    Ok(AnkrTokenBalancesResponse {
        assets,
        next_page_token: result["nextPageToken"].as_str().unwrap_or("").to_string(),
        total_balance_usd: result["totalBalanceUsd"].as_str().unwrap_or("").to_string(),
        error: "".to_string(),
    })
}

fn convert_transaction_result(result: &Value) -> Result<AnkrTransactionsResponse, Status> {
    let transactions = result["transactions"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|tx| {
            let tx_obj = tx.as_object()?;
            Some(crate::api::ankr_transactions_response::Transaction {
                block_hash: tx_obj["blockHash"].as_str().unwrap_or("").to_string(),
                block_number: tx_obj["blockNumber"].as_str().unwrap_or("").to_string(),
                blockchain: tx_obj["blockchain"].as_str().unwrap_or("").to_string(),
                cumulative_gas_used: tx_obj["cumulativeGasUsed"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                from: tx_obj["from"].as_str().unwrap_or("").to_string(),
                gas: tx_obj["gas"].as_str().unwrap_or("").to_string(),
                gas_price: tx_obj["gasPrice"].as_str().unwrap_or("").to_string(),
                gas_used: tx_obj["gasUsed"].as_str().unwrap_or("").to_string(),
                hash: tx_obj["hash"].as_str().unwrap_or("").to_string(),
                input: tx_obj["input"].as_str().unwrap_or("").to_string(),
                nonce: tx_obj["nonce"].as_str().unwrap_or("").to_string(),
                r: tx_obj["r"].as_str().unwrap_or("").to_string(),
                s: tx_obj["s"].as_str().unwrap_or("").to_string(),
                status: tx_obj["status"].as_str().unwrap_or("").to_string(),
                timestamp: tx_obj["timestamp"].as_str().unwrap_or("").to_string(),
                to: tx_obj["to"].as_str().unwrap_or("").to_string(),
                transaction_index: tx_obj["transactionIndex"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                r#type: tx_obj["type"].as_str().unwrap_or("").to_string(), // 优化命名
                v: tx_obj["v"].as_str().unwrap_or("").to_string(),
                value: tx_obj["value"].as_str().unwrap_or("").to_string(),
            })
        })
        .collect::<Vec<_>>();

    Ok(AnkrTransactionsResponse {
        transactions,
        error: "".to_string(),
    })
}

#[derive(Debug, Clone)]
pub struct GrpcService {
    pub state: AppState,
}

// Simple implementation without JsonRpcRequest trait for now
#[tonic::async_trait]
impl AnkrIndexerService for GrpcService {
    async fn proxy_ankr_indexer(
        &self,
        request: Request<AnkrIndexerRequest>,
    ) -> Result<Response<AnkrIndexerResponse>, Status> {
        let req = request.into_inner();
        let params: Value = serde_json::from_slice(&req.params)
            .map_err(|e| Status::invalid_argument(format!("Invalid params: {}", e)))?;

        // For now, we'll use a default provider and chain
        // In a real implementation, these would need to be determined from the request context
        let provider = "ankr"; // Default provider
        let json_request = json!({
            "id": req.id,
            "jsonrpc": &req.jsonrpc,
            "method": &req.method,
            "params": params
        });

        let response = send_indexer_request(&self.state, provider, json_request).await?;
        let result_bytes = serde_json::to_vec(&response)
            .map_err(|e| Status::internal(format!("Failed to serialize result: {}", e)))?;

        let response = AnkrIndexerResponse {
            id: response["id"].as_u64().unwrap_or(req.id),
            jsonrpc: response["jsonrpc"]
                .as_str()
                .unwrap_or(&req.jsonrpc)
                .to_string(),
            result: result_bytes,
        };

        Ok(Response::new(response))
    }

    async fn get_ankr_nfts(
        &self,
        request: Request<AnkrIndexerRequest>,
    ) -> Result<Response<AnkrNftBalancesResponse>, Status> {
        let req = request.into_inner();
        let params: Value = serde_json::from_slice(&req.params)
            .map_err(|e| Status::invalid_argument(format!("Invalid params: {}", e)))?;

        // For now, we'll use a default provider and chain
        let provider = "ankr"; // Default provider
        let json_request = json!({
            "id": req.id,
            "jsonrpc": &req.jsonrpc,
            "method": "getNftList",
            "params": params
        });

        let response = send_indexer_request(&self.state, provider, json_request).await?;
        let result = convert_nft_result(&response["result"])?;

        Ok(Response::new(result))
    }

    async fn get_ankr_tokens(
        &self,
        request: Request<AnkrIndexerRequest>,
    ) -> Result<Response<AnkrTokenBalancesResponse>, Status> {
        let req = request.into_inner();
        let params: Value = serde_json::from_slice(&req.params)
            .map_err(|e| Status::invalid_argument(format!("Invalid params: {}", e)))?;

        // For now, we'll use a default provider and chain
        let provider = "ankr"; // Default provider

        let json_request = json!({
            "id": req.id,
            "jsonrpc": &req.jsonrpc,
            "method": "getTokenList",
            "params": params
        });

        let response = send_indexer_request(&self.state, provider, json_request).await?;
        let result = convert_token_result(&response["result"])?;

        Ok(Response::new(result))
    }

    async fn get_ankr_transactions(
        &self,
        request: Request<AnkrIndexerRequest>,
    ) -> Result<Response<AnkrTransactionsResponse>, Status> {
        let req = request.into_inner();
        let params: Value = serde_json::from_slice(&req.params)
            .map_err(|e| Status::invalid_argument(format!("Invalid params: {}", e)))?;

        // For now, we'll use a default provider and chain
        let provider = "ankr"; // Default provider
        let chain = "ethereum"; // Default chain

        let json_request = json!({
            "id": req.id,
            "jsonrpc": &req.jsonrpc,
            "method": "getTransactionHistory",
            "params": params
        });

        let response = send_jsonrpc_request(&self.state, provider, chain, json_request).await?;
        let result = convert_transaction_result(&response["result"])?;

        Ok(Response::new(result))
    }
}
