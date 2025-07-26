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


### 指标端点

- **路径**: `/metrics`
- **方法**: GET
- **格式**: Prometheus 文本格式
- **访问**: IP限制

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


