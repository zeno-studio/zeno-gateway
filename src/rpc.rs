use crate::api::{RpcRequest, RpcResponse, rpc_service_server::RpcService};
use crate::appstate::AppState;
use crate::common::{handle_jsonrpc_request, JsonRpcRequest};
use serde_json::Value;
use tonic::{Request, Response, Status};

#[derive(Debug, Clone)]
pub struct GrpcService {
    pub state: AppState,
}

#[tonic::async_trait]
impl RpcService for GrpcService {
    async fn proxy_rpc(
        &self,
        request: Request<RpcRequest>,
    ) -> Result<Response<RpcResponse>, Status> {
        let req = request.into_inner();
        let params: Value = serde_json::from_slice(&req.params)
            .map_err(|e| Status::invalid_argument(format!("Invalid params: {}", e)))?;

        let response = handle_jsonrpc_request(
            &self.state,
            &req,
            &req.method,
            params,
            req.id,
            &req.jsonrpc,
            |result| {
                let result_bytes = serde_json::to_vec(result)
                    .map_err(|e| Status::internal(format!("Failed to serialize result: {}", e)))?;
                Ok(RpcResponse {
                    id: 0,
                    jsonrpc: "".to_string(),
                    result: result_bytes,
                })
            },
        )
        .await?;

        Ok(Response::new(response))
    }
}

impl JsonRpcRequest for RpcRequest {
    type Response = RpcResponse;
    fn id(&self) -> u64 { self.id }
    fn jsonrpc(&self) -> &str { &self.jsonrpc }
    fn provider(&self) -> &str { &self.provider }
    fn chain(&self) -> &str { &self.chain }
}
