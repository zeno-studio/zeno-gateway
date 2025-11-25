// src/service.rs
use crate::{
    AppState,
    ankr_types::{
        Balance, GetAccountBalanceReply, GetNFTsByOwnerReply, GetTransactionsByAddressReply, Nft,
        Transaction, Blockchain as AnkrTypesBlockchain,
    },
    pb::ankr::{
        AnkrAssetRequest, AnkrTxHisRequest, AssetsType, HotAsset, HotAssetList,
        TransactionHistoryEntry, TxHistoryList, ankr_indexer_server::AnkrIndexer,
        BlockReference, Blockchain as PbBlockchain,
        block_reference::Kind,
    },
};
use serde_json::Value;
use tonic::{Request, Response, Status};
use std::sync::Arc;
    
pub struct IndexService {
    pub state: Arc<AppState>,
}

fn block_ref_to_json(br: &BlockReference) -> Value {
    match &br.kind {
        Some(Kind::Number(n)) => Value::Number((*n).into()),
        Some(Kind::Latest(_)) => Value::String("latest".into()),
        Some(Kind::Earliest(_)) => Value::String("earliest".into()),
        None => Value::String("latest".into()),
    }
}

fn blockchain_to_chain_id(name: &str) -> u64 {
    match name {
        "eth" => 1,
        "arbitrum" => 42161,
        "optimism" => 10,
        "base" => 8453,
        "linea" => 59144,
        "eth_sepolia" => 11155111,
        _ => 0,
    }
}

fn wei_to_eth(wei_hex: &str) -> f64 {
    if wei_hex.is_empty() || wei_hex == "0x0" || wei_hex == "0" {
        return 0.0;
    }
    let wei = u128::from_str_radix(wei_hex.strip_prefix("0x").unwrap_or(wei_hex), 16).unwrap_or(0);
    wei as f64 / 1e18
}

fn tx_to_entry(tx: Transaction) -> Option<TransactionHistoryEntry> {
    let chain_str = tx.blockchain.as_deref()?.to_lowercase();
    Some(TransactionHistoryEntry {
        tx_hash: tx.hash.unwrap_or_default(),
        block_number: tx.block_number.parse().unwrap_or(0),
        chain_id: blockchain_to_chain_id(&chain_str),
        timestamp: tx.timestamp.as_ref().and_then(|s| s.parse().ok()).unwrap_or(0),
        from: tx.from.clone(),
        to: tx.to.clone().unwrap_or_default(),
        value: tx.value.parse().unwrap_or(0.0),
        gas_price: tx.gas_price.as_ref().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0),
        gas_used: tx.gas_used.as_ref().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0),
    })
}

#[tonic::async_trait]
impl AnkrIndexer for IndexService {
    async fn get_transaction_history(
        &self,
        request: Request<AnkrTxHisRequest>,
    ) -> Result<Response<TxHistoryList>, Status> {
        let req = request.into_inner();
        let mut all_entries = Vec::new();

        // 初始 page_token：如果客户端传 "" 或根本没传，就视为第一页
        let mut current_page_token: Option<String> = if req.page_token.is_empty() {
            None
        } else {
            Some(req.page_token)
        };

        loop {
            let mut body = serde_json::json!({
                "blockchain": req.blockchain.iter().map(|&b| PbBlockchain::try_from(b).unwrap().as_str_name()).collect::<Vec<_>>(),
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

            let ankr_resp: GetTransactionsByAddressReply = self
                .state
                .client
                .post(&endpoint)
                .json(&body)
                .send()
                .await
                .map_err(|e| Status::internal(format!("Ankr request failed: {e}")))?
                .json()
                .await
                .map_err(|e| Status::internal(format!("Ankr parse failed: {e}")))?;

            // 转换当前页数据
            let page_entries = ankr_resp
                .transactions
                .into_iter()
                .filter_map(|tx| tx_to_entry(tx))
                .collect::<Vec<_>>();

            all_entries.extend(page_entries);

            // 判断是否有下一页
            if !ankr_resp.next_page_token.is_empty() {
                current_page_token = Some(ankr_resp.next_page_token.clone());
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
            current_page_token.unwrap_or_default()  // 返回实际的下一页 token
        } else {
            "".to_string()
        };

        Ok(Response::new(TxHistoryList {
            txs: all_entries,
            next_page_token: response_next_token,
        }))
    }
    
    async fn get_asset_balance(
        &self,
        request: Request<AnkrAssetRequest>,
    ) -> Result<Response<HotAssetList>, Status> {
        let req = request.into_inner();
        let endpoint = format!("https://rpc.ankr.com/multichain/{}", self.state.ankr_key);
        
        // 获取余额数据
        let balance_entries = get_balances_by_owner(&self.state.client, &req, &endpoint).await
            .map_err(|e| Status::internal(format!("Get balances failed: {e}")))?;
        
        // 获取 NFT 数据
        let nft_entries = get_nft_by_owner(&self.state.client, &req, &endpoint).await
            .map_err(|e| Status::internal(format!("Get NFTs failed: {e}")))?;
        
        let mut all_entries = balance_entries;
        all_entries.extend(nft_entries);
        
        Ok(Response::new(HotAssetList {
            assets: all_entries,
        }))
    }
}

fn balance_to_asset(address: &str, balance: Balance) -> Option<HotAsset> {
    let chain_str = match &balance.blockchain {
        AnkrTypesBlockchain::Eth => "eth",
        AnkrTypesBlockchain::Arbitrum => "arbitrum",
        AnkrTypesBlockchain::Base => "base",
        AnkrTypesBlockchain::Linea => "linea",
        AnkrTypesBlockchain::Optimism => "optimism",
        AnkrTypesBlockchain::EthSepolia => "eth_sepolia",
    };
    
    let token_type = balance.token_type.clone();
    let token_symbol = balance.token_symbol.clone();
    
    Some(HotAsset {
        chain_id: blockchain_to_chain_id(chain_str),
        address: address.to_string(),
        name: balance.token_name,
        symbol: token_symbol.clone(),
        decimals: balance.token_decimals as u64,
        token_id: 0,
        thumbnail: balance.thumbnail,
        collection: "".to_string(),
        assets_type: map_asset_type(token_type, token_symbol) as i32,
        contract_address: balance.contract_address.clone().unwrap_or_default(),
        balance: balance.balance_usd.parse().unwrap_or(0.0),
        price: balance.token_price.parse().unwrap_or(0.0),
        block_number: 0,
    })
}

fn map_asset_type(asset_type: String, symbol: String) -> AssetsType {
    let asset_type = asset_type.to_lowercase();
    match asset_type.as_str() {
        "erc20" | "erc-20" => AssetsType::Erc20,
        "erc721" | "erc-721" => AssetsType::Erc721,
        "erc1155" | "erc-1155" => AssetsType::Erc1155,
        "undefined" | "unknown" => AssetsType::Unknown,
        _ => {
            // 根据符号判断是否为原生货币
            if symbol.as_str() == "ETH" {
                AssetsType::Currency
            } else {
                AssetsType::Unknown
            }
        }
    }
}

fn map_asset_type2(asset_type: String) -> AssetsType {
    match asset_type.to_lowercase().as_str() {
        "erc721" => AssetsType::Erc721,
        "erc1155" => AssetsType::Erc1155,
        _ => AssetsType::Unknown,
    }
}

fn nft_to_asset(address: &str, nft: Nft) -> Option<HotAsset> {
    let chain_str = match &nft.blockchain {
        AnkrTypesBlockchain::Eth => "eth",
        AnkrTypesBlockchain::Arbitrum => "arbitrum",
        AnkrTypesBlockchain::Base => "base",
        AnkrTypesBlockchain::Linea => "linea",
        AnkrTypesBlockchain::Optimism => "optimism",
        AnkrTypesBlockchain::EthSepolia => "eth_sepolia",
    };
    
    Some(HotAsset {
        chain_id: blockchain_to_chain_id(chain_str),
        address: address.to_string(),
        name: nft.name,
        symbol: nft.symbol,
        decimals: 0,
        token_id: nft.token_id.parse().unwrap_or(0),
        thumbnail: nft.image_url.clone(),
        collection: nft.collection_name.clone(),
        assets_type: map_asset_type2(format!("{:?}", nft.contract_type)) as i32,
        contract_address: nft.contract_address.clone(),
        balance: nft.quantity.as_ref().unwrap_or(&"0".to_string()).parse().unwrap_or(0.0),
        price: 0.0,
        block_number: 0,
    })
}

async fn get_balances_by_owner(
    client: &reqwest::Client,
    request: &AnkrAssetRequest,
    endpoint: &str,
) -> Result<Vec<HotAsset>, Box<dyn std::error::Error>> {
    let mut all_entries = Vec::new();
    
    // 初始 page_token：如果客户端传 "" 或根本没传，就视为第一页
    let mut current_page_token: Option<String> = if request.page_token.is_empty() {
        None
    } else {
        Some(request.page_token.clone())
    };

    loop {
        let mut body = serde_json::json!({
            "blockchain": request.blockchain.iter().map(|&b| PbBlockchain::try_from(b).unwrap().as_str_name()).collect::<Vec<_>>(),
            "address": &request.address[0],
            "onlyWhitelisted": &request.only_whitelisted,
            "pageSize": 50,
        });

        // 只有当 current_page_token 是 Some(非空) 时才加 pageToken 字段
        if let Some(ref token) = current_page_token {
            body["pageToken"] = serde_json::Value::String(token.clone());
        }
        
        let balance_resp: GetAccountBalanceReply = client
            .post(endpoint)
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        // 转换当前页数据
        let page_entries = balance_resp
            .assets
            .into_iter()
            .filter_map(|b| balance_to_asset(&request.address[0], b))
            .collect::<Vec<_>>();

        all_entries.extend(page_entries);

        // 判断是否有下一页
        if let Some(token) = &balance_resp.next_page_token {
            if !token.is_empty() {
                current_page_token = Some(token.clone());
            } else {
                break;
            }
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
) -> Result<Vec<HotAsset>, Box<dyn std::error::Error>> {
    let mut all_entries = Vec::new();

    // 初始 page_token：如果客户端传 "" 或根本没传，就视为第一页
    let mut current_page_token: Option<String> = if request.page_token.is_empty() {
        None
    } else {
        Some(request.page_token.clone())
    };

    loop {
        let mut body = serde_json::json!({
            "blockchain": request.blockchain.iter().map(|&b| PbBlockchain::try_from(b).unwrap().as_str_name()).collect::<Vec<_>>(),
            "address": &request.address[0],
            "pageSize": 50,
        });

        // 只有当 current_page_token 是 Some(非空) 时才加 pageToken 字段
        if let Some(ref token) = current_page_token {
            body["pageToken"] = serde_json::Value::String(token.clone());
        }
        
        let nft_resp: GetNFTsByOwnerReply = client
            .post(endpoint)
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        // 转换当前页数据
        let page_entries = nft_resp
            .assets
            .into_iter()
            .filter_map(|nft| nft_to_asset(&request.address[0], nft))
            .collect::<Vec<_>>();

        all_entries.extend(page_entries);

        if !nft_resp.next_page_token.is_empty() {
            current_page_token = Some(nft_resp.next_page_token.clone());
        } else {
            break;
        }

        if all_entries.len() >= 1000 {
            break;
        }
    }
    
    Ok(all_entries)
}