use crate::pb::auth::auth_service_server::AuthService;  
use crate::pb::auth::{LoginRequest, LoginResponse};  
use crate::state::AppState;

use tonic::{Request, Response, Status};  
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};  
use serde::{Deserialize, Serialize};  
use std::time::{SystemTime, UNIX_EPOCH};  
use std::sync::Arc;



#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,       // device_id（客户端生成 UUID）
    pub iat: usize,        // issued at
    pub exp: usize,        // expiration
}

// 修改 AuthServiceImpl 为包含 AppState 的结构体
#[derive(Clone)]
pub struct AuthServiceImpl {
    pub state: Arc<AppState>,
}

#[tonic::async_trait]
impl AuthService for AuthServiceImpl {
    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let req = request.into_inner();

        // 1. 校验 master api-key（零数据库！）
        if req.api_key != self.state.master_key {
            tracing::warn!("Invalid api_key");
            return Err(Status::unauthenticated("Invalid API Key"));
        }

        // 2. device_id 客户端随便传，只要是合法 UUID 就行（我们不存）
        let device_id = if req.device_id.is_empty() || req.device_id.len() > 128 {
            return Err(Status::invalid_argument("Invalid device_id"));
        } else {
            req.device_id
        };

        // 3. 生成短效 JWT
        let iat = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Status::internal("Time went backwards"))?
            .as_secs() as usize;

        let exp = iat + self.state.token_expires_in;

        let claims = Claims {
            sub: device_id,
            iat,
            exp,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.state.jwt_secret.as_bytes()),
        ).map_err(|e| {
            tracing::error!("JWT encode failed: {}", e);
            Status::internal("Token generation failed")
        })?;

        Ok(Response::new(LoginResponse {
            token,
            expires_in: self.state.token_expires_in as u64,
        }))
    }
}

// ================== 拦截器：零数据库版 ==================
pub fn auth_interceptor(mut req: Request<()>, state: &AppState) -> Result<Request<()>, Status> {
    let token = req.metadata()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| Status::unauthenticated("Missing or invalid token"))?;

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &Validation::new(jsonwebtoken::Algorithm::HS256),
    ).map_err(|e| {
        tracing::debug!("Token invalid: {}", e);
        Status::unauthenticated("Invalid or expired token")
    })?;

    // 把 device_id 塞进 extensions，业务层可以拿来做日志/限流
    req.extensions_mut().insert(token_data.claims);

    Ok(req)
}