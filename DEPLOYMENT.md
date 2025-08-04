# 部署指南

## 概述

本文档描述了如何在VPS上使用podman部署zeno-gateway应用。证书和环境变量文件预先储存在`/etc/ssl/zeno`目录中。

## 架构说明

zeno-gateway应用采用以下架构：

1. **前端与网关通信**：前端通过gRPC与网关通信（端口50051）
2. **网关与Provider通信**：网关通过HTTP与各个Provider通信

```
[前端] ---(gRPC)---> [zeno-gateway] ---(HTTP)---> [Provider]
```

## 部署流程

### 1. 准备环境

确保VPS上已安装podman：
```bash
# Ubuntu/Debian
sudo apt update
sudo apt install podman

# CentOS/RHEL
sudo yum install podman
```

### 2. 创建必要的目录和文件

确保`/etc/ssl/zeno`目录存在，并包含以下文件：
- `cert.pem` - TLS证书
- `key.pem` - TLS私钥
- `.env` - 环境变量配置文件

### 3. 启动gRPC后端服务

在运行zeno-gateway之前，确保gRPC后端服务已经启动并监听正确的地址和端口。

对于本地测试环境：
- 确保gRPC后端服务在`localhost:50051`上运行

对于Docker环境：
- 如果使用docker-compose，确保backend服务已经定义并运行
- 如果单独运行容器，确保gRPC后端服务在`backend:50051`上运行

### 4. 拉取Docker镜像

示例.env文件内容：
```env
# API Keys
ANKR_API_KEY=your_ankr_api_key
BLAST_API_KEY=your_blast_api_key
OPENEXCHANGE_KEY=your_openexchange_key

# Certificate Configuration
TLS_CERT_PATH=./cert.pem
TLS_KEY_PATH=./key.pem

# Server Configuration
# Development: HTTP_PORT=8443 (default)
# Production: HTTP_PORT=443
HTTP_PORT=443

# gRPC Backend Configuration
# 本地测试环境使用: http://localhost:50051
# Docker环境使用: http://backend:50051
GRPC_BACKEND=http://localhost:50051
```

### Docker环境中的域名解析

在Docker环境中，"backend"域名解析通常通过以下方式完成：

1. **Docker自定义网络**：当多个容器在同一个自定义网络中运行时，Docker会自动为每个容器创建基于容器名称的DNS记录。

2. **Docker Compose**：在docker-compose.yml文件中定义服务时，每个服务名称会自动解析为该服务的容器IP。

示例docker-compose.yml文件：
```yaml
version: '3.8'
services:
  zeno-gateway:
    image: ghcr.io/your-username/zeno-gateway:latest
    ports:
      - "443:443"
    volumes:
      - /etc/ssl/zeno:/etc/ssl/zeno:ro
    env_file:
      - /etc/ssl/zeno/.env
    environment:
      - GRPC_BACKEND=http://backend:50051
    networks:
      - app-network
    depends_on:
      - backend

  backend:
    image: ghcr.io/your-username/zeno-backend:latest
    ports:
      - "50051:50051"
    networks:
      - app-network

networks:
  app-network:
    driver: bridge
```

在上述示例中，zeno-gateway服务可以通过"http://backend:50051"访问backend服务，因为它们都在同一个自定义网络"app-network"中运行。

### 3. 拉取Docker镜像

从GitHub Container Registry拉取最新的镜像：
```bash
podman pull ghcr.io/your-username/zeno-gateway:latest
```

### 4. 运行容器

使用podman运行容器，挂载必要的卷并设置环境变量：
```bash
podman run -d \
  --name zeno-gateway \
  -p 443:443 \
  -v /etc/ssl/zeno:/app:ro \
  --env-file /etc/ssl/zeno/.env \
  ghcr.io/your-username/zeno-gateway:latest
```

**参数说明：**
- `-d`：以守护进程模式运行容器
- `--name zeno-gateway`：为容器指定名称
- `-p 443:443`：将宿主机的443端口映射到容器的443端口
- `-v /etc/ssl/zeno:/app:ro`：挂载卷，将宿主机的`/etc/ssl/zeno`目录映射到容器的工作目录`/app`，`:ro`表示以只读模式挂载
- `--env-file /etc/ssl/zeno/.env`：从指定文件加载环境变量
- `ghcr.io/your-username/zeno-gateway:latest`：要运行的镜像名称和标签

**卷挂载说明：**
容器通过卷挂载的方式访问宿主机上的证书和环境变量文件，而不是将这些文件直接拷贝到容器镜像中。这种方式有以下优势：
1. **安全性**：敏感文件不包含在镜像中，防止泄露
2. **灵活性**：可以在不重新构建镜像的情况下更改配置
3. **一致性**：相同的镜像可以在不同环境中使用不同的配置

### 5. 验证部署

检查容器是否正在运行：
```bash
podman ps
```

查看容器日志：
```bash
podman logs zeno-gateway
```

测试服务是否正常工作：
```bash
curl -k https://localhost:443/health
```

有关如何测试各个路由的详细信息，请参阅[TESTING.md](TESTING.md)文件。

## 环境变量说明

| 变量名 | 描述 | 默认值 |
|--------|------|--------|
| ANKR_API_KEY | Ankr API密钥 | 空 |
| BLAST_API_KEY | Blast API密钥 | 空 |
| OPENEXCHANGE_KEY | OpenExchange API密钥 | 空 |
| TLS_CERT_PATH | TLS证书文件路径 | cert.pem |
| TLS_KEY_PATH | TLS私钥文件路径 | key.pem |
| HTTP_PORT | HTTPS服务器端口 | 8443 |
| GRPC_BACKEND | gRPC后端服务地址 | http://localhost:50051 |

## 故障排除

### 证书问题
如果遇到证书相关错误，请检查：
1. `/etc/ssl/zeno/cert.pem` 和 `/etc/ssl/zeno/key.pem` 文件是否存在
2. 文件权限是否正确（podman需要读取权限）
3. 证书是否有效且未过期

### 网络连接问题
如果gRPC连接失败，请检查：
1. `GRPC_BACKEND` 环境变量是否正确设置
2. gRPC服务是否正在运行
3. 网络防火墙设置是否允许相关端口通信

### 日志查看
使用以下命令查看详细日志：
```bash
podman logs -f zeno-gateway
```
