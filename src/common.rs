use serde_json::{json, Value};
use tonic::{Status, Code};
use crate::appstate::AppState;

// JSON-RPC 错误转换为 gRPC Status
pub fn json_error_to_status(error: &Value) -> Result<String, Status> {
    if let Some(e) = error.as_object() {
        if e.is_empty() {
            Ok("".to_string())
        } else {
            serde_json::to_string(e)
                .map_err(|e| Status::internal(format!("Failed to serialize error: {}", e)))
        }
    } else {
        Ok("".to_string())
    }
}

// 发送 JSON-RPC 请求
pub async fn send_jsonrpc_request(
    state: &AppState,
    provider: &str,
    chain: &str,
    json_request: Value,
) -> Result<Value, Status> {
    let endpoint_key = format!("{}_{}", provider, chain);
    let endpoint_url = state
        .rpc_endpoints
        .get(&endpoint_key)
        .ok_or_else(|| Status::not_found("Endpoint not configured"))?;

    state
        .client
        .post(endpoint_url)
        .json(&json_request)
        .send()
        .await
        .map_err(|e| Status::internal(format!("Failed to send request: {}", e)))?
        .json::<Value>()
        .await
        .map_err(|e| Status::internal(format!("Failed to parse response: {}", e)))
}

// 发送 JSON-RPC 请求
pub async fn send_indexer_request(
    state: &AppState,
    provider: &str,
    json_request: Value,
) -> Result<Value, Status> {
     let endpoint_url = match state.indexer_endpoints.get(provider) {
        Some(url) => url.to_owned(),
        None => {
            return Err(Status::not_found(format!("Indexer endpoint for provider {} not found", provider)));
        }
    };

    state
        .client
        .post(endpoint_url)
        .json(&json_request)
        .send()
        .await
        .map_err(|e| Status::internal(format!("Failed to send request: {}", e)))?
        .json::<Value>()
        .await
        .map_err(|e| Status::internal(format!("Failed to parse response: {}", e)))
}


// 构造 JSON-RPC 请求并处理响应
pub async fn handle_jsonrpc_request<T>(
    state: &AppState,
    req: &T,
    method: &str,
    params: Value,
    default_id: u64,
    default_jsonrpc: &str,
    convert_result: impl Fn(&Value) -> Result<T::Response, Status>,
) -> Result<T::Response, Status>
where
    T: JsonRpcRequest,
{
    let json_request = json!({
        "id": req.id(),
        "jsonrpc": req.jsonrpc(),
        "method": method,
        "params": params
    });

    let response = send_jsonrpc_request(state, req.provider(), req.chain(), json_request).await?;
    let _id = response["id"].as_u64().unwrap_or(default_id);
    let _jsonrpc = response["jsonrpc"].as_str().unwrap_or(default_jsonrpc).to_string();
    let error = json_error_to_status(&response["error"])?;

    if !error.is_empty() {
        return Err(Status::new(Code::InvalidArgument, error));
    }

    let result = &response["result"];

    convert_result(result)
}

// JSON-RPC 请求 trait
pub trait JsonRpcRequest {
    type Response;
    fn id(&self) -> u64;
    fn jsonrpc(&self) -> &str;
    fn provider(&self) -> &str;
    fn chain(&self) -> &str;
}
