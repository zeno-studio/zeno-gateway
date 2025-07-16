use async_trait::async_trait;
use pingora::prelude::*;
use pingora_core::services::background::background_service;
use pingora_core::upstreams::peer::HttpPeer;
use pingora_load_balancing::{health_check::HttpHealthCheck, selection::RoundRobin, LoadBalancer};
use pingora_limits::{rate::Rate, scope::Scope};
use pingora::http::{RequestHeader, ResponseHeader, StatusCode};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use std::env;

const ALLOWED_ORIGIN: &str = "https://example.com"; // 替换为你的允许 Origin 域名
const RATE_LIMIT: u32 = 5; // 每秒 5 次请求

// API 密钥（从环境变量获取）
const ANKR_API_KEY: &str = env!("ANKR_API_KEY", "ANKR_API_KEY not set");
const BLAST_API_KEY: &str = env!("BLAST_API_KEY", "BLAST_API_KEY not set");
const GOLDRUSH_API_KEY: &str = env!("GOLDRUSH_API_KEY", "GOLDRUSH_API_KEY not set");

pub struct ApiGateway {
    rpc_upstreams: Arc<LoadBalancer<RoundRobin>>,
    indexer_upstreams: Arc<LoadBalancer<RoundRobin>>,
    forex_peer: HttpPeer,
    rate_limiter: Arc<Rate>,
}

#[async_trait]
impl ProxyHttp for ApiGateway {
    type CTX = ();
    fn new_ctx(&self) -> Self::CTX {}

    async fn request_filter(&self, session: &mut Session, _ctx: &mut Self::CTX) -> Result<bool> {
        let origin = session.get_header("Origin").map(|h| h.to_str().unwrap_or(""));
        if origin != Some(ALLOWED_ORIGIN) {
            session
                .write_response_header(Box::new(ResponseHeader::build(403, None)?))
                .await?;
            session.write_response_body(Some(b"Invalid Origin".to_vec())).await?;
            return Ok(true);
        }

        let client_ip = session.client_addr().ip().to_string();
        if !self.rate_limiter.allow(&client_ip, 1) {
            session
                .write_response_header(Box::new(ResponseHeader::build(429, None)?))
                .await?;
            session.write_response_body(Some(b"Rate Limit Exceeded".to_vec())).await?;
            return Ok(true);
        }

        Ok(false)
    }

    async fn upstream_peer(&self, session: &mut Session, _ctx: &mut ()) -> Result<Box<HttpPeer>> {
        let path = session.req_header().uri.path();
        match path {
            p if p.starts_with("/rpc") => {
                let upstream = self
                    .rpc_upstreams
                    .select(b"rpc", 256)
                    .ok_or_else(|| Error::because(ErrorType::InternalError, "No RPC upstream available"))?;
                Ok(Box::new(HttpPeer::new(upstream, true, "".to_string())))
            }
            p if p.starts_with("/indexer") => {
                let upstream = self
                    .indexer_upstreams
                    .select(b"indexer", 256)
                    .ok_or_else(|| Error::because(ErrorType::InternalError, "No Indexer upstream available"))?;
                Ok(Box::new(HttpPeer::new(upstream, true, "".to_string())))
            }
            p if p.starts_with("/forex") => Ok(Box::new(self.forex_peer.clone())),
            _ => {
                session
                    .write_response_header(Box::new(ResponseHeader::build(404, None)?))
                    .await?;
                session.write_response_body(Some(b"Not Found".to_vec())).await?;
                Err(Error::because(ErrorType::InternalError, "Invalid path"))
            }
        }
    }

    async fn upstream_request_filter(
        &self,
        session: &mut Session,
        upstream_request: &mut RequestHeader,
        _ctx: &mut Self::CTX,
    ) -> Result<()> {
        let path = session.req_header().uri.path();
        match path {
            p if p.starts_with("/rpc") => {
                let upstream_host = self.rpc_upstreams.select(b"rpc", 256).unwrap().0;
                if upstream_host.contains("blastapi.io") {
                    upstream_request
                        .insert_header("Authorization", format!("Bearer {}", BLAST_API_KEY))
                        .map_err(|e| Error::because(ErrorType::InternalError, e.to_string()))?;
                    upstream_request
                        .insert_header("Host", "blastapi.io")
                        .map_err(|e| Error::because(ErrorType::InternalError, e.to_string()))?;
                } else {
                    let new_path = format!("/multichain/{}", ANKR_API_KEY);
                    upstream_request.set_uri(Uri::try_from(new_path)?)?;
                    upstream_request
                        .insert_header("Host", "api.ankr.com")
                        .map_err(|e| Error::because(ErrorType::InternalError, e.to_string()))?;
                }
            }
            p if p.starts_with("/indexer") => {
                let new_path = format!("/indexer/{}", GOLDRUSH_API_KEY);
                upstream_request.set_uri(Uri::try_from(new_path)?)?;
                upstream_request
                    .insert_header("Host", "api.goldrush.io")
                    .map_err(|e| Error::because(ErrorType::InternalError, e.to_string()))?;
            }
            p if p.starts_with("/forex") => {
                upstream_request
                    .insert_header("Host", "forex.yourdomain.com")
                    .map_err(|e| Error::because(ErrorType::InternalError, e.to_string()))?;
            }
            _ => return Err(Error::because(ErrorType::InternalError, "Invalid path")),
        }
        Ok(())
    }
}

fn main() {
    env_logger::init();

    let mut server = Server::new(None).unwrap();
    server.bootstrap();

    let rpc_upstreams = LoadBalancer::try_from_iter([
        "api.ankr.com:443",
        "blastapi.io:443",
    ])
    .unwrap();
    let hc = HttpHealthCheck::new(
        "/health",
        Some(StatusCode::OK),
    );
    rpc_upstreams.set_health_check(hc);
    rpc_upstreams.health_check_frequency = Some(Duration::from_secs(1));
    let rpc_background = background_service("rpc health check", rpc_upstreams);
    let rpc_upstreams = rpc_background.task();

    let indexer_upstreams = LoadBalancer::try_from_iter([
        "api.ankr.com:443",
        "api.goldrush.io:443",
    ])
    .unwrap();
    let hc = HttpHealthCheck::new(
        "/health",
        Some(StatusCode::OK),
    );
    indexer_upstreams.set_health_check(hc);
    indexer_upstreams.health_check_frequency = Some(Duration::from_secs(1));
    let indexer_background = background_service("indexer health check", indexer_upstreams);
    let indexer_upstreams = indexer_background.task();

    let forex_peer = HttpPeer::new(("forex.yourdomain.com:443", false), true, "".to_string());

    let rate_limiter = Arc::new(Rate::new(
        Scope::new(RATE_LIMIT, Duration::from_secs(1)),
    ));

    let mut gateway_service = http_proxy_service(
        &server.configuration,
        ApiGateway {
            rpc_upstreams,
            indexer_upstreams,
            forex_peer,
            rate_limiter,
        },
    );

    // 添加健康检查端点
    gateway_service.add_handler(
        "/health",
        Box::new(|_req| {
            ResponseHeader::build(200, None)
                .map(|h| (h, Some(b"OK".to_vec())))
        }),
    );

    // 添加 Prometheus 指标端点
    gateway_service.add_handler(
        "/metrics",
        Box::new(|_req| {
            let metrics = pingora::metrics::prometheus_metrics();
            ResponseHeader::build(200, None)
                .map(|mut h| {
                    h.append_header("Content-Type", "text/plain; version=0.0.4")?;
                    Ok(h)
                })
                .map(|h| (h, Some(metrics.to_vec())))
        }),
    );

  
    gateway_service.add_tcp("0.0.0.0:8080");

    // 生产环境中启用 TLS
    // let cert_path = "/certs/server.crt";
    // let key_path = "/certs/key.pem";
    // let mut tls_settings = TlsSettings::intermediate(&cert_path, &key_path).unwrap();
    // tls_settings.enable_h2();
    // gateway_service.add_tls_with_settings("0.0.0.0:443", None, tls_settings);

    server.add_service(gateway_service);
    server.run_forever();
}