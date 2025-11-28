// rules.rs
use crate::{
    utils::extract_client_ip,
    client::GLOBAL_STATE};  
use governor::{Quota};  
use std::collections::HashMap;  
use std::num::NonZeroU32;  
use std::sync::RwLock;  
use once_cell::sync::Lazy;  
use tonic::{Request, Status};
use std::pin::Pin;
use std::future::Future;




// 定义一个服务的限流规则  
#[derive(Clone, Debug)]  
pub struct ServiceRule {  
    // 令牌桶配额 (例如: 100 req / 10 min)  
    pub quota: Quota,  
    // 该服务允许的最大并发连接数 (例如: 严格服务要求用户总连接数 <= 2)  
    pub stream_limit: u64,
}  
  
// 全局规则注册表  
pub static RULE_REGISTRY: Lazy<RuleRegistry> = Lazy::new(|| {  
    let mut r = RuleRegistry::new();  
      
    // === 配置规则 1: Metadata Service (普通高频服务) ===  
    // 1分钟 10 次，突发 5 次，允许用户最多开 3 个连接  
    r.register("metadata", ServiceRule {  
        quota: Quota::per_minute(NonZeroU32::new(20).unwrap())  
            .allow_burst(NonZeroU32::new(5).unwrap()),  
        stream_limit: 100,
    });  
  
    // === 配置规则 2: Ankr Service (中等频率服务) ===  
    // 1小时 10 次，突发 3 次，允许用户最多开 1 个连接  
    r.register("ankr", ServiceRule {  
        quota: Quota::per_hour(NonZeroU32::new(10).unwrap())  
            .allow_burst(NonZeroU32::new(3).unwrap()),  
        stream_limit: 50,
    });

    // === 配置规则 4: Price Feed (价格信息服务) ===  
    // 1分钟 10 次，突发 5 次，允许用户最多开 2 个连接  
    r.register("standard", ServiceRule {  
        quota: Quota::per_minute(NonZeroU32::new(10).unwrap())  
            .allow_burst(NonZeroU32::new(5).unwrap()),  
        stream_limit: 200,
    });  
  
    r  
});  
  
pub struct RuleRegistry {  
    rules: RwLock<HashMap<String, ServiceRule>>,  
}  
  
impl RuleRegistry {  
    fn new() -> Self { Self { rules: RwLock::new(HashMap::new()) } }  
      
    fn register(&mut self, name: &str, rule: ServiceRule) {  
        self.rules.write().unwrap().insert(name.to_string(), rule);  
    }  
  
    pub fn get(&self, name: &str) -> Option<ServiceRule> {  
        self.rules.read().unwrap().get(name).cloned()  
    }  
}


#[derive(Clone)]
pub struct RateLimitInterceptor {
    pub rule_name: &'static str,
}

impl tonic_async_interceptor::AsyncInterceptor for RateLimitInterceptor {
    type Future = Pin<Box<dyn Future<Output = Result<Request<()>, Status>> + Send>>;

    fn call(&mut self, req: Request<()>) -> Self::Future {
        let rule_name = self.rule_name;
        let uuid = match req.metadata()
            .get("uuid")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| Status::invalid_argument("Missing UUID metadata"))
            .map(|s| s.to_string()) {
                Ok(uuid) => uuid,
                Err(status) => return Box::pin(async move { Err(status) }),
            };

        if uuid.len() != 128 { 
            return Box::pin(async move { Err(Status::invalid_argument("Invalid UUID")) });
        }

        let ip = extract_client_ip(&req);
        if ip.len() > 45 || ip.len() < 7 { 
            return Box::pin(async move { Err(Status::invalid_argument("Invalid IP format")) });
        }

        Box::pin(async move {
            // 使用异步方式获取客户端状态
            let client_option = GLOBAL_STATE.get_store().get(&uuid).await;
            if let Some(client) = client_option {
                client.try_consume_token(rule_name)
                    .map_err(|e| Status::resource_exhausted(format!("Rate limit exceeded: {}", e)))?;
                GLOBAL_STATE.update_client_state(uuid, ip).await
                    .map_err(|e| Status::internal(format!("Failed to update client state: {}", e)))?;
            } else {
                GLOBAL_STATE.init_client_state(&uuid, &ip, rule_name).await
                    .map_err(|e| Status::internal(format!("Failed to initialize client state: {}", e)))?;
            }

            Ok(req)
        })
    }
}


//客户端示例
// let mut req = tonic::Request::new(AnkrTxHisRequest::default());
// req.metadata_mut().insert("uuid", "user-123".parse().unwrap());
// client.get_tx_history(req).await?;