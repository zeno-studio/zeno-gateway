use std::collections::HashMap;
use axum::{
    body::Body,
    extract::{Path, Request, State},
    http::StatusCode,
    response::Response,
};
use crate::appstate::{AppState, ForexData, RawForexData};
use crate::api::{RpcRequest, IndexerRequest};
use tokio::time::Instant;

pub fn setup_ankr_endpoints(rpc_endpoints: &mut HashMap<String, String>, ankr_key: &str) {
    if ankr_key.is_empty() {
        println!("Warning: ANKR_API_KEY is empty, skipping Ankr endpoints");
        return;
    }

    let chains = vec![
        ("ankr_eth", "eth"),
        ("ankr_bsc", "bsc"),
        ("ankr_arbitrum", "arbitrum"),
        ("ankr_optimism", "optimism"),
        ("ankr_base", "base"),
        ("ankr_polygon", "polygon"),
    ];

    for (endpoint_name, chain) in chains {
        let url = format!("https://rpc.ankr.com/{}/{}", chain, ankr_key);
        rpc_endpoints.insert(endpoint_name.to_string(), url);
    }
}

pub fn setup_blast_endpoints(rpc_endpoints: &mut HashMap<String, String>, blast_key: &str) {
    if blast_key.is_empty() {
        println!("Warning: BLAST_API_KEY is empty, skipping Blast endpoints");
        return;
    }

    let endpoints = vec![
        (
            "blast_eth",
            format!("https://eth-mainnet.blastapi.io/{}", blast_key),
        ),
        (
            "blast_bsc",
            format!("https://bsc-mainnet.blastapi.io/{}", blast_key),
        ),
        (
            "blast_arbitrum",
            format!("https://arbitrum-one.blastapi.io/{}", blast_key),
        ),
        (
            "blast_optimism",
            format!("https://optimism-mainnet.blastapi.io/{}", blast_key),
        ),
        (
            "blast_base",
            format!("https://base-mainnet.blastapi.io/{}", blast_key),
        ),
        (
            "blast_polygon",
            format!("https://polygon-mainnet.blastapi.io/{}", blast_key),
        ),
    ];

    for (endpoint_name, url) in endpoints {
        rpc_endpoints.insert(endpoint_name.to_string(), url);
    }
}

pub fn setup_indexer_endpoints(indexer_endpoints: &mut HashMap<String, String>, ankr_key: &str) {
    if ankr_key.is_empty() {
        println!("Warning: ANKR_API_KEY is empty, skipping indexer endpoints");
        return;
    }

    let endpoints = vec![
        (
            "ankr",
            format!("https://rpc.ankr.com/multichain/{}", ankr_key),
        ),
    ];

    for (endpoint_name, url) in endpoints {
        indexer_endpoints.insert(endpoint_name.to_string(), url);
    }
}

pub async fn proxy_request(
    state: AppState,
    req: Request<Body>,
    endpoint_url: &str,
    path: &str,
    method: &str,
) -> Response<Body> {
    const MAX_BODY_SIZE: usize = 1_000_000; // 1MB
    const MAX_HEADER_COUNT: usize = 50; // Limit to 50 headers
    const MAX_HEADER_SIZE: usize = 1024; // Limit header value to 1KB

    println!("Proxying request: method={}, path={}", method, path);

    let headers = req.headers().clone();
    let method = req.method().clone();

    let body = match axum::body::to_bytes(req.into_body(), MAX_BODY_SIZE).await {
        Ok(body) => body,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::PAYLOAD_TOO_LARGE)
                .body(Body::from("Request body too large (max 1MB)"))
                .unwrap();
        }
    };

    let client = &state.client;
    let mut request_builder = client.request(method, endpoint_url);

    for (name, value) in headers.iter().take(MAX_HEADER_COUNT) {
        if name != "host" && name != "content-length" && value.as_bytes().len() <= MAX_HEADER_SIZE {
            request_builder = request_builder.header(name, value);
        }
    }

    match request_builder.body(body).send().await {
        Ok(response) => {
            let status = response.status();
            let headers = response.headers().clone();

            let body = match response.bytes().await {
                Ok(body) if body.len() <= MAX_BODY_SIZE => body,
                Ok(_) => {
                    return Response::builder()
                        .status(StatusCode::PAYLOAD_TOO_LARGE)
                        .body(Body::from("Response body too large (max 1MB)"))
                        .unwrap();
                }
                Err(e) => {
                    println!("Failed to read response body: {}", e);
                    return Response::builder()
                        .status(StatusCode::BAD_GATEWAY)
                        .body(Body::from(format!("Bad Gateway: {}", e)))
                        .unwrap();
                }
            };

            let mut response_builder = Response::builder().status(status);
            for (name, value) in headers.iter().take(MAX_HEADER_COUNT) {
                if name != "content-length" && name != "transfer-encoding" {
                    response_builder = response_builder.header(name, value);
                }
            }

            response_builder.body(Body::from(body)).unwrap()
        }
        Err(e) => {
            println!("Proxy request failed: {}", e);
            Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(format!("Bad Gateway: {}", e)))
                .unwrap()
        }
    }
}

pub async fn rpc_proxy(
    State(state): State<AppState>,
    Path((provider, chain)): Path<(String, String)>,
    req: Request<Body>,
) -> Response<Body> {
    let start = Instant::now();
    let endpoint_key = format!("{}_{}", provider, chain);

    let body = match axum::body::to_bytes(req.into_body(), 1_000_000).await {
        Ok(body) => body,
        Err(_) => {
            state.metrics.http_requests_total.with_label_values(&["/rpc", "POST", "413"]).inc();
            return Response::builder()
                .status(StatusCode::PAYLOAD_TOO_LARGE)
                .body(Body::from("Request body too large (max 1MB)"))
                .unwrap();
        }
    };

    let mut client = state.rpc_client.clone();
    let request = RpcRequest {
        provider,
        chain,
        body: body.to_vec(),
    };

    match client.proxy_rpc(request).await {
        Ok(response) => {
            let response_body = response.into_inner().body;
            state.metrics.grpc_requests_total.with_label_values(&["RpcService", "ProxyRpc", "200"]).inc();
            state.metrics.grpc_request_duration.with_label_values(&["RpcService", "ProxyRpc", "200"])
                .observe(start.elapsed().as_secs_f64());
            Response::builder()
                .status(StatusCode::OK)
                .body(Body::from(response_body))
                .unwrap()
        }
        Err(e) => {
            state.metrics.grpc_requests_total.with_label_values(&["RpcService", "ProxyRpc", "500"]).inc();
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("gRPC error: {}", e)))
                .unwrap()
        }
    }
}

pub async fn indexer_proxy(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    req: Request<Body>,
) -> Response<Body> {
    let start = Instant::now();
    let body = match axum::body::to_bytes(req.into_body(), 1_000_000).await {
        Ok(body) => body,
        Err(_) => {
            state.metrics.http_requests_total.with_label_values(&["/indexer", "POST", "413"]).inc();
            return Response::builder()
                .status(StatusCode::PAYLOAD_TOO_LARGE)
                .body(Body::from("Request body too large (max 1MB)"))
                .unwrap();
        }
    };

    let mut client = state.indexer_client.clone();
    let request = IndexerRequest {
        provider,
        body: body.to_vec(),
    };

    match client.proxy_indexer(request).await {
        Ok(response) => {
            let response_body = response.into_inner().body;
            state.metrics.grpc_requests_total.with_label_values(&["IndexerService", "ProxyIndexer", "200"]).inc();
            state.metrics.grpc_request_duration.with_label_values(&["IndexerService", "ProxyIndexer", "200"])
                .observe(start.elapsed().as_secs_f64());
            Response::builder()
                .status(StatusCode::OK)
                .body(Body::from(response_body))
                .unwrap()
        }
        Err(e) => {
            state.metrics.grpc_requests_total.with_label_values(&["IndexerService", "ProxyIndexer", "500"]).inc();
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("gRPC error: {}", e)))
                .unwrap()
        }
    }
}

pub async fn forex_data(
    State(state): State<AppState>,
) -> Response<Body> {
    let start = Instant::now();
    let mut client = state.forex_client.clone();
    let request = crate::api::GetForexDataRequest {};

    match client.get_forex_data(request).await {
        Ok(response) => {
            let forex_data = response.into_inner();
            state.metrics.grpc_requests_total.with_label_values(&["ForexService", "GetForexData", "200"]).inc();
            state.metrics.grpc_request_duration.with_label_values(&["ForexService", "GetForexData", "200"])
                .observe(start.elapsed().as_secs_f64());
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&ForexData {
                    timestamp: forex_data.timestamp,
                    rates: forex_data.rates,
                }).unwrap()))
                .unwrap()
        }
        Err(e) => {
            state.metrics.grpc_requests_total.with_label_values(&["ForexService", "GetForexData", "500"]).inc();
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("gRPC error: {}", e)))
                .unwrap()
        }
    }
}

pub async fn raw_forex_data(
    State(state): State<AppState>,
) -> Response<Body> {
    let start = Instant::now();
    let mut client = state.forex_client.clone();
    let request = crate::api::GetRawForexDataRequest {};

    match client.get_raw_forex_data(request).await {
        Ok(response) => {
            let raw_data = response.into_inner();
            state.metrics.grpc_requests_total.with_label_values(&["ForexService", "GetRawForexData", "200"]).inc();
            state.metrics.grpc_request_duration.with_label_values(&["ForexService", "GetRawForexData", "200"])
                .observe(start.elapsed().as_secs_f64());
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&RawForexData {
                    disclaimer: raw_data.disclaimer,
                    license: raw_data.license,
                    timestamp: raw_data.timestamp,
                    base: raw_data.base,
                    rates: raw_data.rates,
                }).unwrap()))
                .unwrap()
        }
        Err(e) => {
            state.metrics.grpc_requests_total.with_label_values(&["ForexService", "GetRawForexData", "500"]).inc();
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("gRPC error: {}", e)))
                .unwrap()
        }
    }
}