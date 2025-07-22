use std::collections::HashMap;

use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    response::Response,
};

use reqwest::Client;

// 应用状态

use crate::appstate::AppState;

// 初始化 Ankr RPC 端点
pub fn setup_ankr_endpoints(rpc_endpoints: &mut HashMap<String, String>, ankr_key: &str) {
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

// 初始化 Blast RPC 端点
pub fn setup_blast_endpoints(rpc_endpoints: &mut HashMap<String, String>, blast_key: &str) {
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

pub async fn rpc_proxy(State(state): State<AppState>, req: Request<Body>) -> Response<Body> {
    let path = req.uri().path().to_string();
    let endpoint_url = match path {
        p if p.starts_with("/rpc/ankr/eth") => state.rpc_endpoints.get("ankr_eth").unwrap(),
        p if p.starts_with("/rpc/ankr/bsc") => state.rpc_endpoints.get("ankr_bsc").unwrap(),
        p if p.starts_with("/rpc/ankr/arbitrum") => {
            state.rpc_endpoints.get("ankr_arbitrum").unwrap()
        }
        p if p.starts_with("/rpc/ankr/optimism") => {
            state.rpc_endpoints.get("ankr_optimism").unwrap()
        }
        p if p.starts_with("/rpc/ankr/base") => state.rpc_endpoints.get("ankr_base").unwrap(),
        p if p.starts_with("/rpc/ankr/polygon") => state.rpc_endpoints.get("ankr_polygon").unwrap(),
        p if p.starts_with("/rpc/blast/eth") => state.rpc_endpoints.get("blast_eth").unwrap(),
        p if p.starts_with("/rpc/blast/bsc") => state.rpc_endpoints.get("blast_bsc").unwrap(),
        p if p.starts_with("/rpc/blast/arbitrum") => {
            state.rpc_endpoints.get("blast_arbitrum").unwrap()
        }
        p if p.starts_with("/rpc/blast/optimism") => {
            state.rpc_endpoints.get("blast_optimism").unwrap()
        }
        p if p.starts_with("/rpc/blast/base") => state.rpc_endpoints.get("blast_base").unwrap(),
        p if p.starts_with("/rpc/blast/polygon") => {
            state.rpc_endpoints.get("blast_polygon").unwrap()
        }
        _ => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not Found"))
                .unwrap();
        }
    };

    // 创建 HTTP 客户端并转发请求
    let client = Client::new();
    let method = req.method().clone();
    let headers = req.headers().clone();
    let body = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .unwrap_or_default();

    let mut request_builder = client.request(method, endpoint_url);

    // 复制请求头
    for (name, value) in headers.iter() {
        if name != "host" && name != "content-length" {
            request_builder = request_builder.header(name, value);
        }
    }

    // 发送请求
    match request_builder.body(body).send().await {
        Ok(response) => {
            let status = response.status();
            let headers = response.headers().clone();
            let body = response.bytes().await.unwrap_or_default();

            let mut response_builder = Response::builder().status(status);

            // 复制响应头
            for (name, value) in headers.iter() {
                if name != "content-length" && name != "transfer-encoding" {
                    response_builder = response_builder.header(name, value);
                }
            }

            response_builder.body(Body::from(body)).unwrap()
        }
        Err(_) => Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Body::from("Bad Gateway"))
            .unwrap(),
    }
}
