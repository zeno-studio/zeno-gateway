// cargo.toml 需要:
// serde = { version = "1.0", features = ["derive"] }
// serde_json = "1.0"

use serde::{Deserialize, Serialize};

/// Blockchain 枚举（强烈推荐使用这个，而不是 String）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Blockchain {
    Arbitrum,
    Base,
    Eth,
    EthSepolia,
    Linea,
    Optimism,
}

/// 用于表示 number | "latest" | "earliest" 这类联合类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BlockReference {
    Number(u64),
    Latest,
    Earliest,
}

impl Default for BlockReference {
    fn default() -> Self {
        BlockReference::Latest
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub timestamp: u64,
    pub lag: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodInput {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub size: u32,
    #[serde(rename = "valueDecoded")]
    pub value_decoded: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Method {
    pub name: String,
    pub inputs: Vec<MethodInput>,
    #[serde(rename = "string")]
    pub string_: String,
    pub signature: String,
    pub id: String,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventInput {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub indexed: bool,
    pub size: u32,
    #[serde(rename = "valueDecoded")]
    pub value_decoded: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub name: String,
    pub inputs: Vec<EventInput>,
    pub anonymous: bool,
    #[serde(rename = "string")]
    pub string_: String,
    pub signature: String,
    pub id: String,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Log {
    pub blockchain: Blockchain,
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
    #[serde(rename = "blockNumber")]
    pub block_number: String,
    #[serde(rename = "transactionHash")]
    pub transaction_hash: String,
    #[serde(rename = "transactionIndex")]
    pub transaction_index: String,
    #[serde(rename = "blockHash")]
    pub block_hash: String,
    #[serde(rename = "logIndex")]
    pub log_index: String,
    pub removed: bool,
    #[serde(default)]
    pub event: Option<Event>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    #[serde(default)]
    pub v: Option<String>,
    #[serde(default)]
    pub r: Option<String>,
    #[serde(default)]
    pub s: Option<String>,
    #[serde(default)]
    pub nonce: Option<String>,
    #[serde(rename = "blockNumber")]
    pub block_number: String,
    pub from: String,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub gas: Option<String>,
    #[serde(default, rename = "gasPrice")]
    pub gas_price: Option<String>,
    #[serde(default)]
    pub input: Option<String>,
    #[serde(rename = "transactionIndex")]
    pub transaction_index: String,
    #[serde(rename = "blockHash")]
    pub block_hash: String,
    pub value: String,
    #[serde(default, rename = "type")] 
    pub type_: Option<String>,
    #[serde(default, rename = "contractAddress")]
    pub contract_address: Option<String>,
    #[serde(default, rename = "cumulativeGasUsed")]
    pub cumulative_gas_used: Option<String>,
    #[serde(default, rename = "gasUsed")]
    pub gas_used: Option<String>,
    #[serde(default)]
    pub logs: Option<Vec<Log>>,
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub blockchain: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub method: Option<Method>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTransactionsByAddressReply {
    pub transactions: Vec<Transaction>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: String,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<SyncStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTransactionsByAddressRequest {
    #[serde(rename = "fromBlock")]
    pub from_block: Option<BlockReference>,
    #[serde(rename = "toBlock")]
    pub to_block: Option<BlockReference>,
    #[serde(rename = "fromTimestamp")]
    pub from_timestamp: Option<BlockReference>,
    #[serde(rename = "toTimestamp")]
    pub to_timestamp: Option<BlockReference>,
    pub blockchain: Vec<Blockchain>,
    pub address: Vec<String>,
    #[serde(rename = "pageToken")]
    pub page_token: Option<String>,
    #[serde(rename = "pageSize")]
    pub page_size: Option<u32>,
    #[serde(rename = "descOrder")]
    pub desc_order: Option<bool>,
    #[serde(rename = "includeLogs")]
    pub include_logs: Option<bool>,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetLogsReply {
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
    pub logs: Vec<Log>,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<SyncStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetLogsRequest {
    #[serde(rename = "fromBlock")]
    pub from_block: Option<BlockReference>,
    #[serde(rename = "toBlock")]
    pub to_block: Option<BlockReference>,
    #[serde(rename = "fromTimestamp")]
    pub from_timestamp: Option<BlockReference>,
    #[serde(rename = "toTimestamp")]
    pub to_timestamp: Option<BlockReference>,
    pub blockchain: Vec<Blockchain>,
    #[serde(default)]
    pub address: Option<Vec<String>>,
    /// topics 可以是 string 或 string[]，所以用 Vec<serde_json::Value> 最灵活
    /// 也可以自定义 enum TopicFilter { Single(String), Multiple(Vec<String>) }
    #[serde(default)]
    pub topics: Option<Vec<serde_json::Value>>,
    #[serde(rename = "pageToken")]
    pub page_token: Option<String>,
    #[serde(rename = "pageSize")]
    pub page_size: Option<u32>,
    #[serde(rename = "descOrder")]
    pub desc_order: Option<bool>,
    #[serde(rename = "decodeLogs")]
    pub decode_logs: Option<bool>,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainStats {
    pub blockchain: String,
    #[serde(rename = "totalTransactionsCount")]
    pub total_transactions_count: u64,
    #[serde(rename = "totalEventsCount")]
    pub total_events_count: u64,
    #[serde(rename = "latestBlockNumber")]
    pub latest_block_number: u64,
    #[serde(rename = "blockTimeMs")]
    pub block_time_ms: u64,
    #[serde(rename = "nativeCoinUsdPrice")]
    pub native_coin_usd_price: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBlockchainStatsReply {
    pub stats: Vec<BlockchainStats>,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<SyncStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBlockchainStatsRequest {
    #[serde(default)]
    pub blockchain: Option<Vec<Blockchain>>,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetInteractionsReply {
    pub blockchains: Vec<String>,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<SyncStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetInteractionsRequest {
    pub address: String,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub blockchain: Blockchain,
    #[serde(rename = "tokenName")]
    pub token_name: String,
    #[serde(rename = "tokenSymbol")]
    pub token_symbol: String,
    #[serde(rename = "tokenDecimals")]
    pub token_decimals: u32,
    #[serde(rename = "tokenType")]
    pub token_type: String,
    #[serde(default, rename = "contractAddress")]
    pub contract_address: Option<String>,
    #[serde(rename = "holderAddress")]
    pub holder_address: String,
    pub balance: String,
    #[serde(rename = "balanceRawInteger")]
    pub balance_raw_integer: String,
    #[serde(rename = "balanceUsd")]
    pub balance_usd: String,
    #[serde(rename = "tokenPrice")]
    pub token_price: String,
    pub thumbnail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAccountBalanceReply {
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
    #[serde(rename = "totalBalanceUsd")]
    pub total_balance_usd: String,
    #[serde(rename = "totalCount")]
    pub total_count: u32,
    pub assets: Vec<Balance>,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<SyncStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAccountBalanceRequest {
    #[serde(default)]
    pub blockchain: Option<Vec<Blockchain>>,
    #[serde(rename = "walletAddress")]
    pub wallet_address: String,
    #[serde(rename = "onlyWhitelisted")]
    pub only_whitelisted: Option<bool>,
    #[serde(rename = "nativeFirst")]
    pub native_first: Option<bool>,
    #[serde(rename = "pageToken")]
    pub page_token: Option<String>,
    #[serde(rename = "pageSize")]
    pub page_size: Option<u32>,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTokenPriceReply {
    #[serde(rename = "usdPrice")]
    pub usd_price: String,
    pub blockchain: Blockchain,
    #[serde(default, rename = "contractAddress")]
    pub contract_address: Option<String>,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<SyncStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTokenPriceRequest {
    pub blockchain: Blockchain,
    #[serde(default, rename = "contractAddress")]
    pub contract_address: Option<String>,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolderBalance {
    #[serde(rename = "holderAddress")]
    pub holder_address: String,
    pub balance: String,
    #[serde(rename = "balanceRawInteger")]
    pub balance_raw_integer: String,
}

use std::collections::HashMap;

// 继续使用上一批中已定义的 Blockchain 和 BlockReference
// pub enum Blockchain { ... }
// pub enum BlockReference { Number(u64), Latest, Earliest }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTokenHoldersReply {
    pub blockchain: Blockchain,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "tokenDecimals")]
    pub token_decimals: u32,
    pub holders: Vec<HolderBalance>,
    #[serde(rename = "holdersCount")]
    pub holders_count: u64,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: String,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<SyncStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTokenHoldersRequest {
    pub blockchain: Blockchain,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "pageToken")]
    pub page_token: Option<String>,
    #[serde(rename = "pageSize")]
    pub page_size: Option<u32>,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyHolderCount {
    #[serde(rename = "holderCount")]
    pub holder_count: u64,
    #[serde(rename = "totalAmount")]
    pub total_amount: String,
    #[serde(rename = "totalAmountRawInteger")]
    pub total_amount_raw_integer: String,
    #[serde(rename = "lastUpdatedAt")]
    pub last_updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTokenHoldersCountReply {
    pub blockchain: Blockchain,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "tokenDecimals")]
    pub token_decimals: u32,
    #[serde(rename = "holderCountHistory")]
    pub holder_count_history: Vec<DailyHolderCount>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: String,
    #[serde(rename = "latestHoldersCount")]
    pub latest_holders_count: u64,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<SyncStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTokenHoldersCountRequest {
    pub blockchain: Blockchain,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "pageToken")]
    pub page_token: Option<String>,
    #[serde(rename = "pageSize")]
    pub page_size: Option<u32>,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyDetailsExtended {
    pub blockchain: Blockchain,
    pub address: Option<String>,
    pub name: String,
    pub decimals: u32,
    pub symbol: String,
    pub thumbnail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCurrenciesReply {
    pub currencies: Vec<CurrencyDetailsExtended>,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<SyncStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCurrenciesRequest {
    pub blockchain: Blockchain,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenTransfer {
    #[serde(rename = "fromAddress")]
    pub from_address: Option<String>,
    #[serde(rename = "toAddress")]
    pub to_address: Option<String>,
    #[serde(rename = "contractAddress")]
    pub contract_address: Option<String>,
    pub value: String,
    #[serde(rename = "valueRawInteger")]
    pub value_raw_integer: String,
    pub blockchain: String,
    #[serde(rename = "tokenName")]
    pub token_name: String,
    #[serde(rename = "tokenSymbol")]
    pub token_symbol: String,
    #[serde(rename = "tokenDecimals")]
    pub token_decimals: u32,
    pub thumbnail: String,
    #[serde(rename = "transactionHash")]
    pub transaction_hash: String,
    #[serde(rename = "blockHeight")]
    pub block_height: u64,
    pub timestamp: u64,
    pub direction: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTokenTransfersReply {
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
    pub transfers: Vec<TokenTransfer>,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<SyncStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTransfersRequest {
    #[serde(rename = "fromBlock")]
    pub from_block: Option<BlockReference>,
    #[serde(rename = "toBlock")]
    pub to_block: Option<BlockReference>,
    #[serde(rename = "fromTimestamp")]
    pub from_timestamp: Option<BlockReference>,
    #[serde(rename = "toTimestamp")]
    pub to_timestamp: Option<BlockReference>,
    pub blockchain: Vec<Blockchain>,
    pub address: Option<Vec<String>>,
    #[serde(rename = "pageToken")]
    pub page_token: Option<String>,
    #[serde(rename = "pageSize")]
    pub page_size: Option<u32>,
    #[serde(rename = "descOrder")]
    pub desc_order: Option<bool>,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trait {
    #[serde(rename = "trait_type")]
    pub trait_type: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractType {
    Erc721,
    Erc1155,
    Undefined,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nft {
    pub blockchain: Blockchain,
    pub name: String,
    #[serde(rename = "tokenId")]
    pub token_id: String,
    #[serde(rename = "tokenUrl")]
    pub token_url: String,
    #[serde(rename = "imageUrl")]
    pub image_url: String,
    #[serde(rename = "collectionName")]
    pub collection_name: String,
    pub symbol: String,
    #[serde(rename = "contractType")]
    pub contract_type: ContractType,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    pub quantity: Option<String>,
    pub traits: Option<Vec<Trait>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNFTsByOwnerReply {
    pub owner: String,
    pub assets: Vec<Nft>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: String,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<SyncStatus>,
}

/// filter 是 { [key: string]: string[] }[]，Rust 中用 Vec<HashMap<String, Vec<String>>>
pub type NftFilter = Vec<HashMap<String, Vec<String>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNFTsByOwnerRequest {
    #[serde(default)]
    pub blockchain: Option<Vec<Blockchain>>,
    pub filter: Option<NftFilter>,
    #[serde(rename = "walletAddress")]
    pub wallet_address: String,
    #[serde(rename = "pageToken")]
    pub page_token: Option<String>,
    #[serde(rename = "pageSize")]
    pub page_size: Option<u32>,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftAttributes {
    #[serde(rename = "tokenUrl")]
    pub token_url: String,
    #[serde(rename = "imageUrl")]
    pub image_url: String,
    pub name: String,
    pub description: String,
    pub traits: Option<Vec<Trait>>,
    #[serde(rename = "contractType")]
    pub contract_type: ContractType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftMetadata {
    pub blockchain: Blockchain,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "tokenId")]
    pub token_id: String,
    #[serde(rename = "contractType")]
    pub contract_type: ContractType,
    #[serde(rename = "collectionName")]
    pub collection_name: String,
    #[serde(rename = "collectionSymbol")]
    pub collection_symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNFTMetadataReply {
    pub metadata: Option<NftMetadata>,
    pub attributes: Option<NftAttributes>,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<SyncStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNFTMetadataRequest {
    pub blockchain: Blockchain,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "tokenId")]
    pub token_id: String,
    #[serde(rename = "forceFetch")]
    pub force_fetch: bool,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNFTHoldersReply {
    pub holders: Vec<String>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: String,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<SyncStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNFTHoldersRequest {
    pub blockchain: Blockchain,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "pageToken")]
    pub page_token: Option<String>,
    #[serde(rename = "pageSize")]
    pub page_size: Option<u32>,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftTransfer {
    #[serde(rename = "fromAddress")]
    pub from_address: String,
    #[serde(rename = "toAddress")]
    pub to_address: String,
    #[serde(rename = "contractAddress")]
    pub contract_address: Option<String>,
    pub value: String,
    #[serde(rename = "tokenId")]
    pub token_id: Option<String>,
    #[serde(rename = "type")]
    pub type_: ContractType,
    pub blockchain: Blockchain,
    #[serde(rename = "transactionHash")]
    pub transaction_hash: String,
    #[serde(rename = "collectionName")]
    pub collection_name: String,
    #[serde(rename = "collectionSymbol")]
    pub collection_symbol: String,
    pub name: String,
    #[serde(rename = "imageUrl")]
    pub image_url: String,
    #[serde(rename = "blockHeight")]
    pub block_height: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNftTransfersReply {
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
    pub transfers: Vec<NftTransfer>,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<SyncStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTokenAllowancesRequest {
    pub blockchain: Vec<Blockchain>,
    #[serde(rename = "walletAddress")]
    pub wallet_address: String,
    #[serde(rename = "spenderAddress")]
    pub spender_address: Option<String>,
    #[serde(rename = "contractAddress")]
    pub contract_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ERC20TokenAllowance {
    #[serde(rename = "walletAddress")]
    pub wallet_address: Option<String>,
    #[serde(rename = "spenderAddress")]
    pub spender_address: Option<String>,
    #[serde(rename = "contractAddress")]
    pub contract_address: Option<String>,
    pub value: Option<String>,
    #[serde(rename = "tokenDecimals")]
    pub token_decimals: Option<u32>,
    #[serde(rename = "blockHeight")]
    pub block_height: u64,
    pub timestamp: u64,
    #[serde(rename = "transactionHash")]
    pub transaction_hash: Option<String>,
    pub blockchain: Option<String>,
    #[serde(rename = "tokenName")]
    pub token_name: Option<String>,
    #[serde(rename = "tokenSymbol")]
    pub token_symbol: Option<String>,
    pub thumbnail: String,
    #[serde(rename = "rawLog")]
    pub raw_log: Option<Log>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTokenAllowancesReply {
    pub allowances: Vec<ERC20TokenAllowance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTokenPriceHistoryRequest {
    pub blockchain: Blockchain,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "fromTimestamp")]
    pub from_timestamp: Option<BlockReference>,
    #[serde(rename = "toTimestamp")]
    pub to_timestamp: Option<BlockReference>,
    pub interval: Option<u64>,
    pub limit: Option<u32>,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub timestamp: u64,
    #[serde(rename = "blockHeight")]
    pub block_height: u64,
    #[serde(rename = "usdPrice")]
    pub usd_price: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTokenPriceHistoryReply {
    pub quotes: Vec<Quote>,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<SyncStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainTokenPriceRequest {
    pub blockchain: Blockchain,
    #[serde(rename = "tokenAddress")]
    pub token_address: String,
    #[serde(rename = "blockHeight")]
    pub block_height: BlockReference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceEstimate {
    pub strategy: String,
    pub price: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainTokenPriceLPDetails {
    pub address: String,
    #[serde(rename = "token0")]
    pub token_0: String,
    #[serde(rename = "token1")]
    pub token_1: String,
    #[serde(rename = "lastUpdatedBlock")]
    pub last_updated_block: u64,
    #[serde(rename = "reserve0")]
    pub reserve_0: String,
    #[serde(rename = "reserve1")]
    pub reserve_1: String,
    pub price: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainTokenPriceTokenDetails {
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    pub decimals: u32,
    pub name: String,
    pub symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainTokenPriceSinglePair {
    #[serde(rename = "token0")]
    pub token_0: ExplainTokenPriceTokenDetails,
    #[serde(rename = "token1")]
    pub token_1: ExplainTokenPriceTokenDetails,
    #[serde(rename = "liquidity_pools")]
    pub liquidity_pools: Vec<ExplainTokenPriceLPDetails>,
    #[serde(rename = "priceEstimates")]
    pub price_estimates: Vec<PriceEstimate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainTokenPriceReply {
    pub blockchain: String,
    #[serde(rename = "tokenAddress")]
    pub token_address: String,
    pub pairs: Vec<ExplainTokenPriceSinglePair>,
    #[serde(rename = "priceEstimates")]
    pub price_estimates: Vec<PriceEstimate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetInternalTransactionsByParentHashRequest {
    pub blockchain: Blockchain,
    #[serde(rename = "parentTransactionHash")]
    pub parent_transaction_hash: String,
    #[serde(rename = "onlyWithValue")]
    pub only_with_value: bool,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetInternalTransactionsByBlockNumberRequest {
    pub blockchain: Blockchain,
    #[serde(rename = "blockNumber")]
    pub block_number: u64,
    #[serde(rename = "onlyWithValue")]
    pub only_with_value: bool,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalTransaction {
    pub blockchain: Blockchain,
    #[serde(rename = "callType")]
    pub call_type: String,
    #[serde(rename = "transactionHash")]
    pub transaction_hash: String,
    #[serde(rename = "blockHeight")]
    pub block_height: u64,
    #[serde(rename = "blockHash")]
    pub block_hash: String,
    #[serde(rename = "fromAddress")]
    pub from_address: String,
    #[serde(rename = "contractAddress")]
    pub contract_address: Option<String>,
    #[serde(rename = "toAddress")]
    pub to_address: String,
    pub value: String,
    pub gas: u64,
    #[serde(rename = "gasUsed")]
    pub gas_used: u64,
    pub timestamp: String,
    #[serde(rename = "transactionIndex")]
    pub transaction_index: u32,
    #[serde(rename = "callPath")]
    pub call_path: Option<String>,
    #[serde(rename = "callStack")]
    pub call_stack: Option<Vec<u32>>,
    pub error: Option<String>,
    pub input: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetInternalTransactionsReply {
    #[serde(rename = "internalTransactions")]
    pub internal_transactions: Vec<InternalTransaction>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAccountBalanceHistoricalRequest {
    #[serde(default)]
    pub blockchain: Option<Vec<Blockchain>>,
    #[serde(rename = "walletAddress")]
    pub wallet_address: String,
    #[serde(rename = "onlyWhitelisted")]
    pub only_whitelisted: Option<bool>,
    #[serde(rename = "nativeFirst")]
    pub native_first: Option<bool>,
    #[serde(rename = "pageToken")]
    pub page_token: Option<String>,
    #[serde(rename = "pageSize")]
    pub page_size: Option<u32>,
    #[serde(rename = "blockHeight")]
    pub block_height: Option<BlockReference>,
    #[serde(rename = "syncCheck")]
    pub sync_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAccountBalanceHistoricalReply {
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
    #[serde(rename = "totalBalanceUsd")]
    pub total_balance_usd: String,
    #[serde(rename = "totalCount")]
    pub total_count: u32,
    pub assets: Vec<Balance>,
    #[serde(rename = "syncStatus")]
    pub sync_status: Option<SyncStatus>,
    #[serde(rename = "blockHeight")]
    pub block_height: Option<BlockReference>,
}