# 测试指南

## 概述

本文档描述了如何在本地环境中测试zeno-gateway应用的gRPC服务。

## 准备工作

### 1. 启动应用

确保应用已在本地运行：
```bash
cargo run
```

默认情况下，gRPC服务将在`localhost:50051`上监听。

### 2. 准备测试工具

可以使用以下工具测试gRPC服务：
- grpcurl: 命令行工具，类似于curl但用于gRPC
- BloomRPC: 图形化gRPC测试工具
- grpcui: Web界面的gRPC测试工具

## gRPC服务测试

### 1. 安装grpcurl

```bash
# macOS
brew install grpcurl

# Ubuntu/Debian
sudo apt install grpcurl

# 或者从源码安装
go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest
```

### 2. 列出可用服务

```bash
grpcurl -plaintext localhost:50051 list
```

预期输出:
```
api.LatestForexService
api.AnkrIndexerService
api.RpcService
grpc.reflection.v1alpha.ServerReflection
```

### 3. 列出服务方法

```bash
# 列出LatestForexService的方法
grpcurl -plaintext localhost:50051 list api.LatestForexService

# 列出RpcService的方法
grpcurl -plaintext localhost:50051 list api.RpcService

# 列出AnkrIndexerService的方法
grpcurl -plaintext localhost:50051 list api.AnkrIndexerService
```

### 4. 测试LatestForexService

```bash
# 获取处理后的外汇数据
grpcurl -plaintext localhost:50051 api.LatestForexService/GetForexData

# 获取原始外汇数据
grpcurl -plaintext localhost:50051 api.LatestForexService/GetLatestForexData
```

### 5. 测试RpcService

```bash
# 向Ethereum主网发送RPC请求
grpcurl -plaintext -d '{
  "provider": "ankr",
  "chain": "eth",
  "body": "{\"jsonrpc\":\"2.0\",\"method\":\"eth_blockNumber\",\"params\":[],\"id\":1}"
}' localhost:50051 api.RpcService/ProxyRpc
```

### 6. 测试AnkrIndexerService

```bash
# 向Ankr索引器发送请求
grpcurl -plaintext -d '{
  "provider": "ankr",
  "body": "{\"jsonrpc\":\"2.0\",\"method\":\"ankr_getAccountBalance\",\"params\":[{\"blockchain\":[\"eth\"],\"walletAddress\":\"0x...\",\"nativeFirst\":true}],\"id\":1}"
}' localhost:50051 api.AnkrIndexerService/ProxyAnkrIndexer
```

## 支持的Provider和Chain

### RpcService支持的provider:
- ankr
- blast

### RpcService支持的chain:
- eth (Ethereum)
- bsc (Binance Smart Chain)
- polygon
- fantom
- avalanche
- arbitrum
- optimism
- cronos

### AnkrIndexerService支持的provider:
- ankr

## 故障排除

### 1. 连接被拒绝

确保应用正在运行并且gRPC服务监听在正确的端口。

### 2. 服务未找到

检查服务名称是否正确，可以使用`list`命令查看可用服务。

### 3. 方法未找到

检查方法名称是否正确，可以使用`list <service>`命令查看服务的方法。

### 4. 参数错误

检查请求参数是否符合proto定义。

### 5. 服务器错误

查看应用日志以获取更多错误信息。
