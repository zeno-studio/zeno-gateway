// client.rs

use crate::rules::{RULE_REGISTRY};  
use dashmap::DashMap;  
use governor::{RateLimiter, state::direct::NotKeyed, clock::DefaultClock};  
use moka::future::Cache;  
use once_cell::sync::Lazy;  
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};  
use std::time::{Duration, Instant};  
use tonic::Status;  

// 类型别名：具体的令牌桶类型  
type SharedBucket = Arc<RateLimiter<NotKeyed, governor::state::InMemoryState, DefaultClock>>;

// 单个用户的状态  
pub struct ClientState {  
    // Sticky IP  
    pub bound_ip: Mutex<Option<String>>,  
    // 连接是否活跃
    is_connected: AtomicBool,
    // 动态桶：Key 是服务名 (如 "ankr_index")  
    pub buckets: DashMap<String, SharedBucket>,
    // 最后活跃时间，用于心跳检测
    last_active: Mutex<Instant>,
}

impl ClientState {  
    fn new() -> Self {  
        Self {  
            bound_ip: Mutex::new(None),  
            is_connected: AtomicBool::new(false),
            buckets: DashMap::new(),  
            last_active: Mutex::new(Instant::now()),
        }  
    }  
    
    // 更新最后活跃时间
    pub fn update_last_active(&self) {
        *self.last_active.lock().unwrap() = Instant::now();
    }
    
    // 检查连接是否超时（超过60秒无活动）
    pub fn is_expired(&self) -> bool {
        let last = self.last_active.lock().unwrap();
        last.elapsed() > Duration::from_secs(60)
    }
    
    // 标记连接为活跃状态
    pub fn mark_connected(&self) {
        self.is_connected.store(true, Ordering::Release);
    }
    
    // 标记连接为断开状态
    pub fn mark_disconnected(&self) {
        self.is_connected.store(false, Ordering::Release);
    }
    
    // 检查连接是否活跃
    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::Acquire)
    }

    // 获取(或懒加载)指定服务的令牌桶  
    pub fn get_bucket_for_service(&self, service_name: &str) -> Result<SharedBucket, Status> {
        // 如果已经存在，直接返回  
        if let Some(bucket) = self.buckets.get(service_name) {  
            return Ok(bucket.value().clone());  // 使用 value() 方法获取 Arc 内容
        }  

        // 如果不存在，查找全局配置并创建  
        let rule = RULE_REGISTRY.get(service_name)  
            .ok_or_else(|| Status::internal(format!("Rule not found for service: {}", service_name)))?;

        // 创建新桶  
        let new_bucket = Arc::new(RateLimiter::direct(rule.quota));  
        self.buckets.insert(service_name.to_string(), new_bucket.clone());  
          
        Ok(new_bucket)  
    }
    
    // 尝试扣除指定服务的令牌
    pub fn try_consume_token(&self, service_name: &str) -> Result<(), Status> {
        let bucket = self.get_bucket_for_service(service_name)?;
        
        // 检查并消费一个令牌，如果失败则返回错误
        bucket.check().map_err(|_| Status::resource_exhausted(format!("Rate limit exceeded for service: {}", service_name)))?;
        Ok(())
    }
     
}  

// 全局用户状态缓存  
pub static GLOBAL_STATE: Lazy<GlobalStateManager> = Lazy::new(GlobalStateManager::new);  

// 全局活跃连接列表，用于心跳检测
pub static ACTIVE_CONNECTIONS: Lazy<DashMap<String, Instant>> = Lazy::new(DashMap::new);

pub struct GlobalStateManager {  
    // 10分钟无操作自动过期  
    store: Cache<String, Arc<ClientState>>,  
}

impl GlobalStateManager {  
    fn new() -> Self {  
        Self {  
            store: Cache::builder()  
                .time_to_idle(Duration::from_secs(600)) // 10分钟 idle 清除  
                .build(),  
        }  
    }  
    
    // 处理连接请求，验证UUID并建立ClientState
    pub async fn update_client_state(&self, uuid: String, ip: String) -> Result<(), Status> {
  
        let state = self.store.get_with(uuid.clone(), async { Arc::new(ClientState::new()) }).await;
        state.update_last_active();
        ACTIVE_CONNECTIONS.insert(uuid.clone(), Instant::now());
        let mut ip_guard = state.bound_ip.lock().unwrap();  
        if let Some(ref bound) = *ip_guard {  
            if bound != &ip {  
                return Err(Status::permission_denied("UUID bound to different IP"));  
            }  
        } else {  
            *ip_guard = Some(ip);  
        }  
        drop(ip_guard); 
        state.mark_connected();
        Ok(())  
    }

    pub async fn init_client_state(&self, uuid: &str, ip: &str, service_name: &str) -> Result<(), Status> {
        let rule = RULE_REGISTRY.get(service_name)  
            .ok_or_else(|| Status::internal(format!("Rule not found for service: {}", service_name)))?;  
        let new_bucket = Arc::new(RateLimiter::direct(rule.quota));  
        
        let client_state = ClientState{
            bound_ip: Mutex::new(Some(ip.to_string())),
            is_connected: AtomicBool::new(true),
            buckets: {
                let buckets = DashMap::new();
                buckets.insert(service_name.to_string(), new_bucket);
                buckets
            },
            last_active: Mutex::new(Instant::now()),
        };
        self.store.insert(uuid.to_string(), Arc::new(client_state)).await;
        // 注意：client_state 是 ClientState 的实例，不是 Arc 包装的
        // 我们需要从 store 中获取 Arc 包装的实例来调用方法
        if let Some(stored_client_state) = self.store.get(uuid).await {
            stored_client_state.mark_connected();
            stored_client_state.update_last_active();
            ACTIVE_CONNECTIONS.insert(uuid.to_string(), Instant::now());
        }
        Ok(())
    }
  

  
    // 连接断开时调用  
    pub async fn release_conn(&self, uuid: &str) {  
        if let Some(state) = self.store.get(uuid).await {  
            state.mark_disconnected();
            // 从活跃连接列表中移除
            ACTIVE_CONNECTIONS.remove(uuid);
            // moka 会自动处理 time_to_idle  
        }  
    }  

    // 获取缓存存储，用于外部清理任务
    pub fn get_store(&self) -> &Cache<String, Arc<ClientState>> {
        &self.store
    }
    
    // 清理过期连接
    pub async fn cleanup_expired_connections(&self) {
        let now = Instant::now();
        let mut expired_uuids = Vec::new();
        
        // 找出所有过期的连接
        for entry in ACTIVE_CONNECTIONS.iter() {
            let uuid = entry.key();
            let last_active = entry.value();
            
            if now.duration_since(*last_active) > Duration::from_secs(60) {
                expired_uuids.push(uuid.clone());
            }
        }
        
        // 清理过期的连接
        for uuid in expired_uuids {
            println!("Cleaning up expired connection for UUID: {}", uuid);
            ACTIVE_CONNECTIONS.remove(&uuid);
            // 注意：这里我们不直接从缓存中移除，让moka自己处理
            // 如果需要立即移除，可以调用 self.store.invalidate(&uuid).await;
            // 对于 Tonic 后台连接信息的清理，需要在服务层实现特定的连接断开机制
        }
    }
    
    // 检查连接是否仍然有效
    pub async fn is_connection_valid(&self, uuid: &str) -> bool {
        if let Some(state) = self.store.get(uuid).await {
            state.is_connected()
        } else {
            false
        }
    }
    
    // 强制断开连接（包括清理 Tonic 后台连接信息）
    // 注意：此函数需要与服务层配合使用，通过特定机制通知 Tonic 断开连接
    pub async fn force_disconnect(&self, uuid: &str) {
        if let Some(state) = self.store.get(uuid).await {
            state.mark_disconnected();
            ACTIVE_CONNECTIONS.remove(uuid);
            // 这里可以添加与服务层通信的机制，通知 Tonic 断开特定连接
            // 具体实现取决于服务层的设计
        }
    }
}