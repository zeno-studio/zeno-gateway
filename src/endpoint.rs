use std::collections::HashMap;

use axum::{
    body::Body,
    extract::{Path, Request, State},
    http::StatusCode,
    response::Response,
};


use crate::appstate::AppState;

// Initialize Ankr RPC endpoints
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

// Initialize Blast RPC endpoints
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

// Initialize indexer endpoints
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

// Shared proxy logic
async fn proxy_request(
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

    // Clone headers before consuming req
    let headers = req.headers().clone();
    let method = req.method().clone();

    // Read request body with size limit
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

    // Copy request headers with limits
    for (name, value) in headers.iter().take(MAX_HEADER_COUNT) {
        if name != "host" && name != "content-length" && value.as_bytes().len() <= MAX_HEADER_SIZE {
            request_builder = request_builder.header(name, value);
        }
    }

    // Send request
    match request_builder.body(body).send().await {
        Ok(response) => {
            let status = response.status();
            let headers = response.headers().clone();

            // Read response body with size limit
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
            // Copy response headers with limits
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
    let endpoint_key = format!("{}_{}", provider, chain);
    let endpoint_url = match state.rpc_endpoints.get(&endpoint_key) {
        Some(url) => url.to_owned(),
        None => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Endpoint not configured"))
                .unwrap();
        }
    };

    let path = req.uri().path().to_string();
    let method = req.method().as_str().to_owned();
    let endpoint_url_clone = endpoint_url.clone();
    
    proxy_request(state, req, &endpoint_url_clone, &path, &method).await
}

pub async fn indexer_proxy(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    req: Request<Body>,
) -> Response<Body> {
    let endpoint_url = match state.indexer_endpoints.get(&provider) {
        Some(url) => url.to_owned(),
        None => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Endpoint not configured"))
                .unwrap();
        }
    };

    let path = req.uri().path().to_string();
    let method = req.method().as_str().to_owned();
    let endpoint_url_clone = endpoint_url.clone();
    
    proxy_request(state, req, &endpoint_url_clone, &path, &method).await
}