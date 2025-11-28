use tonic::{Request, transport::server::TcpConnectInfo};
use rustls::ServerConfig;
use crate::error::Result;
/// 从 tonic 的 Request 中万无一失地提取真实客户端 IP
/// 支持顺序：X-Forwarded-For > X-Real-IP > Forwarded > 直连对端IP
pub fn extract_client_ip<T>(req: &Request<T>) -> String {
    // 1. 优先读取标准 header（从右到左第一个可信 IP）
    if let Some(xff) = req.metadata().get("x-forwarded-for") {
        if let Ok(xff_str) = xff.to_str() {
            // X-Forwarded-For: client_ip, proxy1, proxy2
            let ips: Vec<&str> = xff_str.split(',').map(|s| s.trim()).collect();
            if let Some(first) = ips.first() {
                if let Ok(ip) = first.parse::<std::net::IpAddr>() {
                    return ip.to_string();
                }
            }
        }
    }

    // 2. X-Real-IP（Nginx/Traefik 常用）
    if let Some(real_ip) = req.metadata().get("x-real-ip") {
        if let Ok(s) = real_ip.to_str() {
            if let Ok(ip) = s.trim().parse::<std::net::IpAddr>() {
                return ip.to_string();
            }
        }
    }

    // 3. Forwarded 标准 header（RFC 7239）
    if let Some(forwarded) = req.metadata().get("forwarded") {
        if let Ok(s) = forwarded.to_str() {
            // 示例: For="[2001:db8::1]:1234", for=192.0.2.60;proto=http;by=203.0.113.43
            for pair in s.split(';') {
                let pair = pair.trim();
                if pair.to_lowercase().starts_with("for=") {
                    let ip_part = pair[4..].trim_matches(|c| c == '"' || c == '[' || c == ']');
                    // 可能带端口，如 192.0.2.1:54321 或 [2001:db8::1]:1234
                    let ip = ip_part.split(':').next().unwrap_or(ip_part);
                    if let Ok(addr) = ip.parse::<std::net::IpAddr>() {
                        return addr.to_string();
                    }
                }
            }
        }
    }

    // 4. 最后兜底：tonic 内置的直连对端地址（本地调试或无代理时使用）
    if let Some(connect_info) = req.extensions().get::<TcpConnectInfo>() {
        if let Some(addr) = connect_info.remote_addr {
            return addr.ip().to_string();
        }
    }

    // 理论上走不到这里
    "0.0.0.0".to_string()
}


/// 辅助函数：从内存字节构建 Rustls ServerConfig  
pub fn load_rustls_config(cert: &[u8], key: &[u8]) -> Result<ServerConfig> {
    let mut cert_reader = std::io::Cursor::new(cert);
    let certs =
        rustls_pemfile::certs(&mut cert_reader).collect::<std::result::Result<Vec<_>, _>>()?;

    let mut key_reader = std::io::Cursor::new(key);
    // 尝试解析 PKCS8，如果实际是 RSA 或其他格式，可按需添加 fallback
    let keys: Vec<rustls::pki_types::PrivateKeyDer> =
        rustls_pemfile::pkcs8_private_keys(&mut key_reader)
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(rustls::pki_types::PrivateKeyDer::from)
            .collect();

    let private_key = keys.into_iter().next().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "No private keys found")
    })?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, private_key)
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to create server config: {}", e),
            )
        })?;

    Ok(config)
}
