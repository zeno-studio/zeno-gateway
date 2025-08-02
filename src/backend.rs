use tonic::{transport::Server, Request, Response, Status};
use crate::api::{
    forex_service_server::{ForexService, ForexServiceServer},
    rpc_service_server::{RpcService, RpcServiceServer},
    indexer_service_server::{IndexerService, IndexerServiceServer},
    ForexDataResponse, RawForexDataResponse, RpcRequest, RpcResponse, IndexerRequest, IndexerResponse,
};
use crate::appstate::AppState;
use crate::endpoint::proxy_request;
use axum::body::{Body, to_bytes};


#[derive(Debug, Clone)]
struct BackendService {
    state: AppState,
}

#[tonic::async_trait]
impl ForexService for BackendService {
    async fn get_forex_data(
        &self,
        _request: Request<crate::api::GetForexDataRequest>,
    ) -> Result<Response<ForexDataResponse>, Status> {
        let forex_data = self.state.forex_data.read().await;
        Ok(Response::new(ForexDataResponse {
            timestamp: forex_data.timestamp,
            rates: forex_data.rates.clone(),
        }))
    }

    async fn get_raw_forex_data(
        &self,
        _request: Request<crate::api::GetRawForexDataRequest>,
    ) -> Result<Response<RawForexDataResponse>, Status> {
        let raw_data = self.state.raw_forex_data.read().await;
        if let Some(data) = raw_data.as_ref() {
            Ok(Response::new(RawForexDataResponse {
                disclaimer: data.disclaimer.clone(),
                license: data.license.clone(),
                timestamp: data.timestamp,
                base: data.base.clone(),
                rates: data.rates.clone(),
            }))
        } else {
            Err(Status::not_found("Raw forex data not available"))
        }
    }
}


#[tonic::async_trait]
impl RpcService for BackendService {
    async fn proxy_rpc(
        &self,
        request: Request<RpcRequest>,
    ) -> Result<Response<RpcResponse>, Status> {
        let req = request.into_inner();
        let endpoint_key = format!("{}_{}", req.provider, req.chain);
        let endpoint_url = self.state.rpc_endpoints.get(&endpoint_key)
            .ok_or_else(|| Status::not_found("Endpoint not configured"))?;

        let http_req = http::Request::builder()
            .method("POST")
            .uri(endpoint_url)
            .header("content-type", "application/json")
            .body(Body::from(req.body.clone()))
            .unwrap();
        let response = proxy_request(
            self.state.clone(),
            http_req,
            endpoint_url,
            &format!("/rpc/{}/{}", req.provider, req.chain),
            "POST",
        ).await;

        let body = to_bytes(response.into_body(), 1_000_000).await.unwrap(); // 使用 axum::body::to_bytes
        Ok(Response::new(RpcResponse { body: body.to_vec() }))
    }
}

#[tonic::async_trait]
impl IndexerService for BackendService {
    async fn proxy_indexer(
        &self,
        request: Request<IndexerRequest>,
    ) -> Result<Response<IndexerResponse>, Status> {
        let req = request.into_inner();
        let endpoint_url = self.state.indexer_endpoints.get(&req.provider)
            .ok_or_else(|| Status::not_found("Endpoint not configured"))?;

        let http_req = http::Request::builder()
            .method("POST")
            .uri(endpoint_url)
            .header("content-type", "application/json")
            .body(Body::from(req.body.clone()))
            .unwrap();
        let response = proxy_request(
            self.state.clone(),
            http_req,
            endpoint_url,
            &format!("/indexer/{}", req.provider),
            "POST",
        ).await;

        let body = to_bytes(response.into_body(), 1_000_000).await.unwrap(); // 使用 axum::body::to_bytes
        Ok(Response::new(IndexerResponse { body: body.to_vec() }))
    }
}

pub async fn start_backend(state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:50051".parse()?;
    let service = BackendService { state };
    let forex_server = ForexServiceServer::new(service.clone());
    let rpc_server = RpcServiceServer::new(service.clone());
    let indexer_server = IndexerServiceServer::new(service);

    Server::builder()
        .add_service(forex_server)
        .add_service(rpc_server)
        .add_service(indexer_server)
        .serve(addr)
        .await?;
    Ok(())
}