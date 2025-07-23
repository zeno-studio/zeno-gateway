# Zeno Gateway

一个高性能的区块链RPC网关，支持多链路由、外汇数据服务和Prometheus监控。

## 功能特性

- **多链RPC支持**: 支持Ankr和Blast的RPC服务
- **自动HTTPS**: 使用ACME协议自动申请和更新Let's Encrypt证书
- **速率限制**: 基于IP的智能速率限制
- **外汇数据**: 实时外汇汇率数据
- **Prometheus监控**: 完整的指标收集和监控
- **健康检查**: 服务健康状态监控

## 快速开始

### 1. 环境配置

复制环境变量模板：
```bash
cp .env.example .env
```

编辑 `.env` 文件，填入必要的配置：
```bash
# API Keys
ANKR_API_KEY=your_ankr_api_key_here
BLAST_API_KEY=your_blast_api_key_here
OPENEXCHANGE_KEY=your_openexchange_key_here

# 域名配置（用于HTTPS证书）
DOMAIN=yourdomain.com
ACME_CONTACT=mailto:admin@yourdomain.com
```

### 2. 运行服务

#### 生产环境（自动HTTPS）
```bash
cargo run
```
服务将自动申请Let's Encrypt证书并在8443端口提供HTTPS服务。

#### 开发环境（HTTP）
```bash
# 在.env文件中设置
ENABLE_HTTPS=false

cargo run
```
服务将在3000端口提供HTTP服务。

### 3. 访问服务

- **HTTPS模式**: https://yourdomain.com:8443
- **HTTP模式**: http://localhost:3000

## API端点

### RPC服务
- `GET/POST /rpc/ankr/{path}` - Ankr RPC服务
- `GET/POST /rpc/blast/{path}` - Blast RPC服务

### 外汇数据
- `GET /forex` - 获取外汇汇率数据
- `GET /forex/raw` - 获取原始外汇数据

### 监控
- `GET /health` - 健康检查
- `GET /metrics` - Prometheus指标

### 代理服务
- `GET/POST /indexer/{path}` - 索引器代理服务

## ACME证书管理

本项目使用[rustls-acme](https://github.com/FlorianUekermann/rustls-acme)实现自动证书管理：

### 配置选项

- **DOMAIN**: 必需，用于申请证书的域名
- **ACME_CONTACT**: 可选，联系邮箱（默认：mailto:admin@example.com）
- **ACME_DIRECTORY**: 可选，ACME服务器地址
  - 生产环境：https://acme-v02.api.letsencrypt.org/directory
  - 测试环境：https://acme-staging-v02.api.letsencrypt.org/directory
- **ACME_CACHE_DIR**: 可选，证书缓存目录（默认：./acme-cache）

### 首次运行

首次运行时会自动申请证书，需要：
1. 域名已正确解析到服务器IP
2. 服务器80端口可访问（用于HTTP-01验证）
3. 有效的联系邮箱

### 证书更新

证书会在到期前30天自动更新，无需人工干预。

## Docker部署

```dockerfile
FROM rust:1.80 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/zeno-gateway /app/
COPY .env.example /app/.env.example
EXPOSE 8443
CMD ["./zeno-gateway"]
```

## 环境变量参考

| 变量名 | 描述 | 默认值 |
|--------|------|--------|
| `ANKR_API_KEY` | Ankr API密钥 | - |
| `BLAST_API_KEY` | Blast API密钥 | - |
| `OPENEXCHANGE_KEY` | OpenExchange API密钥 | - |
| `DOMAIN` | 域名（用于HTTPS证书） | - |
| `ACME_CONTACT` | ACME联系邮箱 | mailto:admin@example.com |
| `ACME_DIRECTORY` | ACME服务器地址 | https://acme-v02.api.letsencrypt.org/directory |
| `ACME_CACHE_DIR` | 证书缓存目录 | ./acme-cache |
| `ENABLE_HTTPS` | 是否启用HTTPS | true |

## 开发

```bash
# 运行测试
cargo test

# 代码格式化
cargo fmt

# 代码检查
cargo clippy
```

## 许可证

MIT License
