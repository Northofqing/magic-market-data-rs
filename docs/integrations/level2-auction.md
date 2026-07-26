# 授权 Level-2 集合竞价接入

本项目的 `Auctions` 是完整集合竞价合同，不是普通 Quote 的别名。一个可用记录必须由
同一授权来源同时提供：

- 证券代码和名称；
- 撮合价、昨收和涨跌幅；
- 匹配数量、匹配金额；
- 未匹配买量、未匹配卖量；
- 量比；
- Provider 源时间、观察时间和批次证据。

普通网页、五档快照和逐笔成交不能证明未匹配队列。抓取完成时间也不能替代源时间。
因此 TDX、腾讯、SSE/SZSE 公网页面和未授权终端缺少上述字段时必须继续返回 typed
`Unsupported`，不得用零值或推算结果补齐。

## Provider 接入步骤

授权数据适配器应在自己的 Provider crate 中实现
`magic_market_core::Auctions`。本仓不要求 Router 依赖具体厂商：

```rust
use magic_market_core::{
    verify_auction_conformance, Auctions, InstrumentId, ProviderId,
};

fn verify<P>(
    provider: &P,
    instruments: &[InstrumentId],
    provider_id: ProviderId,
) -> Result<(), Box<dyn std::error::Error>>
where
    P: Auctions,
    P::Error: 'static,
{
    let batch = provider.auction_snapshots(instruments)?;
    verify_auction_conformance(instruments, provider_id, &batch)?;
    Ok(())
}
```

`verify_auction_conformance` 强制校验非空且无重复的精确请求、精确返回数量、完整质量、
代码覆盖、Provider、批次 ID、记录/批次源时间一致和所有完整字段。它只验证合同，
不会替厂商授予许可，也不会自动把 capability 改成 true。

## 凭据和准入

- 凭据通过厂商 SDK、密钥服务或进程环境注入，不写入仓库、fixture、日志或批次证据；
- 不读取浏览器 Cookie，不复用个人网页会话；
- 厂商 endpoint、账户/终端身份、重连和限流策略由具体 Provider 封装；
- deterministic conformance、受控实网探针、许可审查和人工证据全部通过后，才可广告
  `auction=true`；
- 只有部分字段的公共观察必须使用另一个明确命名的 diagnostic 类型，不能实现或广告
  完整 `Auctions`。

## 必测边界

至少覆盖缺名称、缺未匹配任一侧、缺量比、缺源时间、未来/陈旧时间、证券错配、重复
证券、记录/批次 Provider 或 batch ID 冲突、部分成功和登录失效。任何一项失败都不得
生成可用集合竞价批次。
