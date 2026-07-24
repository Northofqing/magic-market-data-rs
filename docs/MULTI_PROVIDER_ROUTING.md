# 多数据源路由

`magic-market-router` 在各 Provider 的标准化 Core 适配器之上提供顺序切源。它只
接受第一个满足证据政策的完整批次，不把不同来源的证券或分页拼成一个成功批次。

## 路由流程

```text
同一不可变请求
    │
    ├─ Provider A ─ 终止错误 ───────────────→ 整体失败
    │              可恢复错误/质量拒绝 ─┐
    ├─ Provider B ←──────────────────────┘
    │              合格批次 ─────────────→ 返回批次 + 完整 attempt trace
    └─ Provider C ─ 全部失败 ─────────────→ Exhausted + 完整 attempt trace
```

每个数据族使用独立的 `FailoverChain`。当前别名覆盖 Quote、K 线、分时、逐笔、
资金流、盘后资金流排行、五档、集合竞价、证券元数据、行情统计、技术 K 线、
研报/一致预期/语义搜索、板块与信号、龙虎榜/人气、资本事件、新闻/公告/互动、
财报、涨跌停池和 ETF 期权。生产 crate 只依赖 `magic-market-core`；TDX、
Tencent、Sina、EMQuant、公共情报及交易所官方 Provider 都通过相同的薄适配器注册。

## 错误分类

Provider 的错误必须在注册点映射为 `SourceError`，不能解析错误显示文本决定策略：

| 源错误 | `FailureKind` | `FailureAction` |
| --- | --- | --- |
| 重复代码、非法范围、超出请求上限 | `InvalidRequest` | `Stop` |
| 当前市场/周期/字段不支持 | `Unsupported` | `TryNext` |
| DNS、连接、断线 | `Transport` | `TryNext` |
| 明确超时 | `Timeout` | `TryNext` |
| 限频 | `RateLimited` | `TryNext` |
| 空响应 | `NoData` | `TryNext` |
| 解码、协议或字段矛盾 | `Protocol` | 由调用方显式选择 |
| 权限或其他 Provider 错误 | `Provider` | 由调用方显式选择 |

非法调用请求必须停止。否则同一个错误在后续 Provider “成功”会掩盖调用方缺陷。
EMQuant `10001003/EQERR_NO_ACCESS` 和
`10001012/EQERR_ACCESS_INSUFFICIENCE` 都应保留为 Provider 权限错误；账号能登录
或某个数据族查询成功，不代表其他 capability 已经获得上线权限。

## 接受政策与强制证据门

`AcceptancePolicy` 有两个可选门：

- `require_complete`：拒绝 `QualityReport` 含问题的批次；
- `require_source_at`：拒绝批次级 `source_at=None`。

无论政策如何配置，路由始终拒绝：

- 空成功批次；
- 记录 `ProviderId` 与注册来源不同；
- provenance 缺少批次 ID；
- 记录批次 ID 与 provenance 批次 ID 不同。

路由不解析时间字符串或使用一个固定秒数判断所有数据族。Quote、日线、盘后指标
和集合竞价的时间语义不同，调用方应在选中批次后执行自己的交易阶段新鲜度门。
特别是 `PostCloseFlowRouter` 还会验证请求日期、批次来源日期、记录交易日、请求
上限以及排名/证券唯一性，不会把普通日级或板块资金流改名成 15:35 Top10。
`AnnouncementRouter` 同样校验请求证券、发布日期范围、记录来源日期、调用上限和
公告 ID 唯一性，官方交易所源与 CNInfo 不能靠错误归属通过路由质量门。

## 使用

```rust
use magic_market_core::ProviderId;
use magic_market_router::{
    quote_source, AcceptancePolicy, FailureKind, QuoteRouter, SourceError,
};
use magic_tencent_rs::{TencentClient, TencentError};
use std::sync::Arc;

let mut router = QuoteRouter::new(
    AcceptancePolicy::new()
        .with_require_complete(true)
        .with_require_source_at(true),
);
router.register(quote_source(
    ProviderId::Tencent,
    Arc::new(TencentClient::new()?),
    |error| match error {
        TencentError::InvalidRequest(message) => {
            SourceError::stop(FailureKind::InvalidRequest, message)
        }
        TencentError::Unsupported(message) => {
            SourceError::try_next(FailureKind::Unsupported, message)
        }
        TencentError::Transport(message) => {
            SourceError::try_next(FailureKind::Transport, message)
        }
        other => SourceError::try_next(FailureKind::Protocol, other.to_string()),
    },
))?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

成功时读取 `RouteOutcome::attempts()` 和 `selected_provider()`；失败时读取
`RouterError::attempts()`。这些 attempt 应进入调用方监控/审计，但不得包含账号、
手机号、密码、激活令牌或原始登录报文。

## 实盘验收

2026-07-23 执行：

```bash
cargo run -p magic-market-router --example live_probe --release --locked --offline
```

默认顺序为 TDX、Tencent，并要求完整质量和来源时间。真实结果是：

- TDX 成功返回 Quote，但因证券名称缺失、Quote 源时间格式未验证而被
  `FailureKind::Quality` 拒绝；
- Tencent 被选中，华电辽能 `600396.SH` 返回价格 16.22，来源时间
  `2026-07-23T13:49:34+08:00`；
- attempt trace 同时保留 TDX 拒绝与 Tencent 选中；
- `router_live_probe_status=passed`。

这个结果证明质量门和切源实际运行，不代表腾讯网页端点具有生产 SLA。

## 明确边界

路由不增加 Provider 内部重试，不缓存旧响应，不跨源合并，不运行后台线程，不保存
行情，也不提供 HTTP 服务。熔断冷却、并发预算、数据库、定时拉取、持续监控和
`stock_analysis` 的五秒新鲜度政策仍由下游服务实现。
