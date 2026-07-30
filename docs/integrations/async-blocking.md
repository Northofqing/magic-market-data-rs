# 在异步服务中调用同步 Provider

当前 HTTP Provider 客户端执行同步阻塞 I/O：共享
`magic-market-transport` 使用 `reqwest::blocking`，其余已登记 Provider 仍可能
使用同步 `ureq`；`RequestGate::wait_for_turn` 节流也会阻塞当前线程。完整边界见
[`http-transports.tsv`](http-transports.tsv)。

不要在 Tokio executor worker 上直接调用这些客户端。把客户端 clone、请求数据和
阻塞调用一起移入 `tokio::task::spawn_blocking`：

```rust
use magic_market_core::{
    AssetClass, Exchange, InstrumentId, RealtimeQuotes,
};
use magic_tencent_rs::{TencentClient, TencentError};

async fn tencent_quote(
    client: TencentClient,
    instrument: InstrumentId,
) -> Result<magic_market_core::DataBatch<magic_market_core::Quote>, Box<dyn std::error::Error>> {
    let batch = tokio::task::spawn_blocking(move || {
        client.realtime_quotes(&[instrument])
    })
    .await? // JoinError：任务 panic 或 runtime 关闭
    .map_err(|error: TencentError| -> Box<dyn std::error::Error> {
        Box::new(error)
    })?; // Provider typed error
    Ok(batch)
}

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let client = TencentClient::new()?;
let instrument =
    InstrumentId::new(Exchange::Shanghai, "600000", AssetClass::Equity)?;
let batch = tencent_quote(client, instrument).await?;
# let _ = batch;
# Ok(())
# }
```

`spawn_blocking` 只保护异步 executor，不提供请求取消：future 被丢弃后，已经开始的
socket 调用仍会运行到完成或超时。必须给客户端配置有界连接、读、写超时。

常驻服务还应在外层用 `Semaphore`、有界工作队列或服务级并发限制约束
`spawn_blocking` 数量；不要把 Tokio 的 blocking 线程池当作 Provider 限频器。业务
服务继续负责熔断、缓存、持久化和指标，本仓库保留 typed failure 与 provenance。
