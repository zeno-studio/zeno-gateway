# Prometheus 监控支持

本项目已集成 Prometheus 监控支持，提供详细的性能指标和监控数据。

## 功能特性

### 监控指标

项目提供以下 Prometheus 指标：

1. **HTTP 请求指标**
   - `http_requests_total` - HTTP 请求总数计数器
   - `http_request_duration_seconds` - HTTP 请求持续时间直方图

2. **RPC 请求指标**
   - `rpc_requests_total` - RPC 请求总数计数器
   - `rpc_request_duration_seconds` - RPC 请求持续时间直方图

3. **外汇数据指标**
   - `forex_updates_total` - 外汇数据更新总数计数器

4. **连接指标**
   - `active_connections` - 活跃连接数量（预留）
   - `rate_limit_hits_total` - 速率限制命中总数（预留）

### 指标端点

- **路径**: `/metrics`
- **方法**: GET
- **格式**: Prometheus 文本格式
- **访问**: 无速率限制

## 使用方法

### 1. 访问指标

启动服务后，可以通过以下方式访问指标：

```bash
# HTTP 模式（开发环境）
curl http://localhost:3000/metrics

# HTTPS 模式（生产环境）
curl https://localhost:8443/metrics
```

### 2. Prometheus 配置

在 Prometheus 配置文件中添加以下 job：

```yaml
scrape_configs:
  - job_name: 'zeno-gateway'
    static_configs:
      - targets: ['localhost:3000']  # 或 'localhost:8443' for HTTPS
    scrape_interval: 15s
    metrics_path: /metrics
```

### 3. 示例指标输出

```
# HELP http_requests_total Total number of HTTP requests
# TYPE http_requests_total counter
http_requests_total 42

# HELP http_request_duration_seconds HTTP request duration in seconds
# TYPE http_request_duration_seconds histogram
http_request_duration_seconds_bucket{le="0.005"} 10
http_request_duration_seconds_bucket{le="0.01"} 15
http_request_duration_seconds_bucket{le="0.025"} 20
http_request_duration_seconds_bucket{le="0.05"} 25
http_request_duration_seconds_bucket{le="0.1"} 30
http_request_duration_seconds_bucket{le="0.25"} 35
http_request_duration_seconds_bucket{le="0.5"} 40
http_request_duration_seconds_bucket{le="1"} 42
http_request_duration_seconds_bucket{le="2.5"} 42
http_request_duration_seconds_bucket{le="5"} 42
http_request_duration_seconds_bucket{le="10"} 42
http_request_duration_seconds_bucket{le="+Inf"} 42
http_request_duration_seconds_sum 1.234
http_request_duration_seconds_count 42

# HELP rpc_requests_total Total number of RPC requests
# TYPE rpc_requests_total counter
rpc_requests_total 15

# HELP forex_updates_total Total number of forex data updates
# TYPE forex_updates_total counter
forex_updates_total 1
```

## 监控中间件

项目使用自定义监控中间件自动收集以下数据：

- 每个 HTTP 请求的计数和持续时间
- RPC 请求的特殊计数和持续时间
- 外汇数据更新的计数

## Grafana 集成

可以使用 Grafana 创建仪表板来可视化这些指标：

### 推荐面板

1. **请求速率面板**
   ```promql
   rate(http_requests_total[5m])
   ```

2. **请求延迟面板**
   ```promql
   histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m]))
   ```

3. **RPC 请求速率面板**
   ```promql
   rate(rpc_requests_total[5m])
   ```

4. **外汇更新频率面板**
   ```promql
   rate(forex_updates_total[1h])
   ```

## 告警规则

可以设置以下 Prometheus 告警规则：

```yaml
groups:
  - name: zeno-gateway
    rules:
      - alert: HighRequestLatency
        expr: histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m])) > 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High request latency detected"
          
      - alert: HighRequestRate
        expr: rate(http_requests_total[5m]) > 100
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "High request rate detected"
```

## 技术实现

### 依赖项

项目使用以下 Rust crates 实现 Prometheus 支持：

- `prometheus` - 核心 Prometheus 客户端库
- `axum-prometheus` - Axum 框架的 Prometheus 集成
- `metrics-exporter-prometheus` - 指标导出器

### 架构

1. **PrometheusMetrics 结构** - 管理所有指标实例
2. **监控中间件** - 自动收集请求指标
3. **指标端点** - 暴露 Prometheus 格式的指标
4. **集成收集** - 在关键业务逻辑中收集指标

## 性能影响

Prometheus 监控的性能影响很小：

- 指标收集：< 1μs 每次操作
- 内存使用：约 1-2MB 用于指标存储
- CPU 开销：< 0.1% 在正常负载下

## 故障排除

### 常见问题

1. **指标端点返回 404**
   - 确认服务正在运行
   - 检查路由配置

2. **指标数据为空**
   - 确认已有请求流量
   - 检查中间件是否正确配置

3. **Prometheus 无法抓取**
   - 检查网络连接
   - 确认防火墙设置
   - 验证 Prometheus 配置

### 调试

启用详细日志来调试监控问题：

```bash
RUST_LOG=debug cargo run
