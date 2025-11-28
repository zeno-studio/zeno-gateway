// src/ankr.rs
use crate::{
    error::{AppError, Result},
    pb::ankr::{
        AnkrAssetRequest, AnkrTxHisRequest, BlockReference, Blockchain as PbBlockchain, HotAsset,
        HotAssetList, TransactionHistoryEntry, TxHistoryList, ankr_indexer_server::AnkrIndexer,
        block_reference::Kind,
    },
    state::IndexService,
};
use serde_json::Value;
use tonic::{Request, Response, Status};

// 辅助函数：将Blockchain枚举转换为小写字符串名称，并跳过BLOCKCHAIN_UNDEFINED
fn blockchain_to_str(blockchain: &i32) -> Option<String> {
    if let Ok(pb_blockchain) = PbBlockchain::try_from(*blockchain) {
        // 跳过BLOCKCHAIN_UNDEFINED
        if !matches!(pb_blockchain, PbBlockchain::Undefined) {
            // 转换为小写字符串
            return Some(pb_blockchain.as_str_name().to_lowercase());
        }
    }
    None
}

fn block_ref_to_json(br: &BlockReference) -> Value {
    match &br.kind {
        Some(Kind::Number(n)) => Value::Number((*n).into()),
        Some(Kind::Latest(_)) => Value::String("latest".into()),
        Some(Kind::Earliest(_)) => Value::String("earliest".into()),
        None => Value::String("latest".into()),
    }
}

// 直接从JSON值转换为TransactionHistoryEntry
fn tx_json_to_entry(tx_json: &Value) -> Option<TransactionHistoryEntry> {
    Some(TransactionHistoryEntry {
        tx_hash: tx_json.get("hash")?.as_str().unwrap_or("").to_string(),
        block_number: tx_json
            .get("blockNumber")?
            .as_str()
            .unwrap_or("0")
            .to_string(),
        blockchain: tx_json
            .get("blockchain")?
            .as_str()
            .unwrap_or("0")
            .to_string(),
        timestamp: tx_json
            .get("timestamp")?
            .as_str()
            .unwrap_or("0")
            .to_string(),
        from: tx_json.get("from")?.as_str().unwrap_or("").to_string(),
        to: tx_json
            .get("to")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        value: tx_json.get("value")?.as_str().unwrap_or("0").to_string(),
        gas_price: tx_json
            .get("gasPrice")
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .to_string(),
        gas_used: tx_json
            .get("gasUsed")
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .to_string(),
    })
}

#[tonic::async_trait]
impl AnkrIndexer for IndexService {
    async fn get_transaction_history(
        &self,
        request: Request<AnkrTxHisRequest>,
    ) -> std::result::Result<Response<TxHistoryList>, Status> {
        match self.get_transaction_history_internal(request.into_inner()).await {
            Ok(response) => Ok(response),
            Err(e) => Err(Status::internal(format!("Error: {}", e))),
        }
    }

    async fn get_asset_balance(
        &self,
        request: Request<AnkrAssetRequest>,
    ) -> std::result::Result<Response<HotAssetList>, Status> {

        match self.get_asset_balance_internal(request.into_inner()).await {
            Ok(response) => Ok(response),
            Err(e) => Err(Status::internal(format!("Error: {}", e))),
        }
    }
}

impl IndexService {
    async fn get_transaction_history_internal(
        &self,
        req: AnkrTxHisRequest,
    ) -> Result<Response<TxHistoryList>> {
        let mut all_entries = Vec::new();

        // 初始 page_token：如果客户端传 "" 或根本没传，就视为第一页
        let mut current_page_token: Option<String> = if req.page_token.is_empty() {
            None
        } else {
            Some(req.page_token)
        };

        loop {
            // 过滤掉None值并收集有效的区块链名称
            let blockchain_names: Vec<String> = req
                .blockchain
                .iter()
                .filter_map(|&b| blockchain_to_str(&b))
                .collect();

            let mut body = serde_json::json!({
                "blockchain": blockchain_names,
                "address": &req.address[0],
                "decodeTxData": true,
                "includeLogs": false,
                "descOrder": true,
                "pageSize": 100,
            });

            // 只有当 current_page_token 是 Some(非空) 时才加 pageToken 字段
            if let Some(ref token) = current_page_token {
                body["pageToken"] = serde_json::Value::String(token.clone());
            }

            if let Some(ref from) = req.from_timestamp {
                body["fromTimestamp"] = block_ref_to_json(from);
            }
            if let Some(ref to) = req.to_timestamp {
                body["toTimestamp"] = block_ref_to_json(to);
            }

            let endpoint = format!("https://rpc.ankr.com/multichain/{}", self.state.ankr_key);

            // 直接获取JSON响应，而不反序列化为结构体
            let ankr_resp: Value = self
                .state
                .client
                .post(&endpoint)
                .json(&body)
                .send()
                .await
                .map_err(AppError::from)?
                .json()
                .await
                .map_err(AppError::from)?;

            // 直接从JSON中提取交易数据
            if let Some(transactions) = ankr_resp.get("transactions").and_then(|t| t.as_array()) {
                let page_entries = transactions
                    .iter()
                    .filter_map(|tx_json| tx_json_to_entry(tx_json))
                    .collect::<Vec<_>>();

                all_entries.extend(page_entries);
            }

            // 判断是否有下一页
            let next_page_token = ankr_resp
                .get("nextPageToken")
                .and_then(|t| t.as_str())
                .unwrap_or("");

            if !next_page_token.is_empty() {
                current_page_token = Some(next_page_token.to_string());
            } else {
                // 没有下一页，退出循环
                current_page_token = None;
                break;
            }

            if all_entries.len() >= 10_000 {
                break;
            }
        }

        // 返回给客户端的 next_page_token：如果有更多数据，返回下一页的 token，否则返回空字符串
        let response_next_token = if current_page_token.is_some() {
            current_page_token.unwrap_or_default() // 返回实际的下一页 token
        } else {
            "".to_string()
        };

        Ok(Response::new(TxHistoryList {
            txs: all_entries,
            next_page_token: response_next_token,
        }))
    }

    async fn get_asset_balance_internal(
        &self,
        req: AnkrAssetRequest,
    ) -> Result<Response<HotAssetList>> {
        let endpoint = format!("https://rpc.ankr.com/multichain/{}", self.state.ankr_key);

        // 获取余额数据
        let balance_entries = get_balances_by_owner(&self.state.client, &req, &endpoint).await?;

        // 获取 NFT 数据
        let nft_entries = get_nft_by_owner(&self.state.client, &req, &endpoint).await?;

        let mut all_entries = balance_entries;
        all_entries.extend(nft_entries);

        Ok(Response::new(HotAssetList {
            assets: all_entries,
        }))
    }
}

// 直接从JSON值转换为HotAsset (余额)
fn balance_json_to_asset(address: &str, balance_json: &Value) -> Option<HotAsset> {
    Some(HotAsset {
        blockchain: balance_json
            .get("blockchain")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        address: address.to_string(),
        name: balance_json
            .get("tokenName")?
            .as_str()
            .unwrap_or("")
            .to_string(),
        symbol: balance_json.get("tokenSymbol")?.as_str()?.to_string(),
        decimals: balance_json
            .get("tokenDecimals")?
            .as_u64()
            .unwrap_or(0)
            .to_string(),
        token_id: "".to_string(),
        thumbnail: balance_json
            .get("thumbnail")?
            .as_str()
            .unwrap_or("")
            .to_string(),
        collection: "".to_string(),
        assets_type: balance_json
            .get("tokenType")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        contract_address: balance_json
            .get("contractAddress")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        balance: balance_json
            .get("balanceUsd")?
            .as_str()
            .unwrap_or("0")
            .to_string(),
        price: balance_json
            .get("tokenPrice")?
            .as_str()
            .unwrap_or("0")
            .to_string(),
    })
}

// 直接从JSON值转换为HotAsset (NFT)
fn nft_json_to_asset(address: &str, nft_json: &Value) -> Option<HotAsset> {
    Some(HotAsset {
        blockchain: nft_json
            .get("blockchain")?
            .as_str()
            .unwrap_or("")
            .to_string(),
        address: address.to_string(),
        name: nft_json.get("name")?.as_str().unwrap_or("").to_string(),
        symbol: nft_json.get("symbol")?.as_str().unwrap_or("").to_string(),
        decimals: "".to_string(),
        token_id: nft_json.get("tokenId")?.as_str().unwrap_or("0").to_string(),
        thumbnail: nft_json
            .get("imageUrl")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        collection: nft_json
            .get("collectionName")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        assets_type: nft_json
            .get("contractType")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        contract_address: nft_json
            .get("contractAddress")?
            .as_str()
            .unwrap_or("")
            .to_string(),
        balance: nft_json
            .get("quantity")
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .to_string(),
        price: "".to_string(),
    })
}

async fn get_balances_by_owner(
    client: &reqwest::Client,
    request: &AnkrAssetRequest,
    endpoint: &str,
) -> Result<Vec<HotAsset>> {
    let mut all_entries = Vec::new();

    // 初始 page_token：如果客户端传 "" 或根本没传，就视为第一页
    let mut current_page_token: Option<String> = if request.page_token.is_empty() {
        None
    } else {
        Some(request.page_token.clone())
    };

    loop {
        // 过滤掉None值并收集有效的区块链名称
        let blockchain_names: Vec<String> = request
            .blockchain
            .iter()
            .filter_map(|&b| blockchain_to_str(&b))
            .collect();

        let mut body = serde_json::json!({
            "blockchain": blockchain_names,
            "address": &request.address[0],
            "onlyWhitelisted": &request.only_whitelisted,
            "pageSize": 50,
        });

        // 只有当 current_page_token 是 Some(非空) 时才加 pageToken 字段
        if let Some(ref token) = current_page_token {
            body["pageToken"] = serde_json::Value::String(token.clone());
        }

        // 直接获取JSON响应，而不反序列化为结构体
        let balance_resp: Value = client
            .post(endpoint)
            .json(&body)
            .send()
            .await
            .map_err(AppError::from)?
            .json()
            .await
            .map_err(AppError::from)?;

        // 直接从JSON中提取余额数据
        if let Some(assets) = balance_resp.get("assets").and_then(|t| t.as_array()) {
            let page_entries = assets
                .iter()
                .filter_map(|balance_json| balance_json_to_asset(&request.address[0], balance_json))
                .collect::<Vec<_>>();

            all_entries.extend(page_entries);
        }

        // 判断是否有下一页
        let next_page_token = balance_resp
            .get("nextPageToken")
            .and_then(|t| t.as_str())
            .unwrap_or("");

        if !next_page_token.is_empty() {
            current_page_token = Some(next_page_token.to_string());
        } else {
            break;
        }

        if all_entries.len() >= 1000 {
            break;
        }
    }

    Ok(all_entries)
}

async fn get_nft_by_owner(
    client: &reqwest::Client,
    request: &AnkrAssetRequest,
    endpoint: &str,
) -> Result<Vec<HotAsset>> {
    let mut all_entries = Vec::new();

    // 初始 page_token：如果客户端传 "" 或根本没传，就视为第一页
    let mut current_page_token: Option<String> = if request.page_token.is_empty() {
        None
    } else {
        Some(request.page_token.clone())
    };

    loop {
        // 过滤掉None值并收集有效的区块链名称
        let blockchain_names: Vec<String> = request
            .blockchain
            .iter()
            .filter_map(|&b| blockchain_to_str(&b))
            .collect();

        let mut body = serde_json::json!({
            "blockchain": blockchain_names,
            "address": &request.address[0],
            "pageSize": 50,
        });

        // 只有当 current_page_token 是 Some(非空) 时才加 pageToken 字段
        if let Some(ref token) = current_page_token {
            body["pageToken"] = serde_json::Value::String(token.clone());
        }

        // 直接获取JSON响应，而不反序列化为结构体
        let nft_resp: Value = client
            .post(endpoint)
            .json(&body)
            .send()
            .await
            .map_err(AppError::from)?
            .json()
            .await
            .map_err(AppError::from)?;

        // 直接从JSON中提取NFT数据
        if let Some(assets) = nft_resp.get("assets").and_then(|t| t.as_array()) {
            let page_entries = assets
                .iter()
                .filter_map(|nft_json| nft_json_to_asset(&request.address[0], nft_json))
                .collect::<Vec<_>>();

            all_entries.extend(page_entries);
        }

        // 判断是否有下一页
        let next_page_token = nft_resp
            .get("nextPageToken")
            .and_then(|t| t.as_str())
            .unwrap_or("");

        if !next_page_token.is_empty() {
            current_page_token = Some(next_page_token.to_string());
        } else {
            break;
        }

        if all_entries.len() >= 1000 {
            break;
        }
    }

    Ok(all_entries)
}