# 东方财富公开网页数据接入

`magic-eastmoney-rs` 是与 Choice/EMQuant 完全分离的只读公开网页 Provider。它不读取
东方财富桌面客户端、Cookie、账号、设备激活信息或交易数据，也不会借用
`magic-emquant-rs` 的登录态。

## 已实现的数据族

| 数据族 | 标准化入口 | 已实现范围 |
| --- | --- | --- |
| 研报 | `ResearchReports` | 个股、行业研报，作者、评级、行业、盈利预测和 PDF URL |
| 个股资金流 | `FundFlowSeries` | 分钟与日级主力/超大/大/中/小单净流入 |
| 板块资金流 | `BoardFlows` | 行业、概念、地域的涨跌、分档净流入和领涨股 |
| 龙虎榜 | `DragonTigerData` | 个股上榜明细与营业部买卖净额 |
| 资本数据 | `MarginData`、`BlockTrades`、`HolderCounts`、`LockupEvents`、`DividendPlans` | 融资融券、大宗交易、股东户数、限售解禁、分红送转 |
| 打板 | `LimitPools` | 涨停、炸板、跌停、昨日涨停 |
| 热度 | `PopularityData` | 当前人气排名，并保留榜单与行情的两份证据 |
| 关键词新闻诊断 | `NewsProvider::instrument_news` | 响应无结构化证券身份，capability 为 false 且正式调用返回 `Unsupported` |

未实现的能力不会由相近字段推测：研报 PDF 只返回源 URL，不声称已下载；涨停原因
capability 当前为 false；15:35 盘后资金流 Top10 没有经过验证的源端语义，因此
`post_close_flow` 为 false。

## 网络与安全边界

实现只允许 HTTPS 443，并限制到以下东方财富主机：

```text
reportapi.eastmoney.com
push2.eastmoney.com
push2his.eastmoney.com
push2ex.eastmoney.com
datacenter-web.eastmoney.com
emappdata.eastmoney.com
```

研报记录可以返回 `pdf.dfcfw.com` 的外部 PDF URL，但本 crate 不请求或下载该主机；
它不属于生产 transport allowlist。

HTTP 客户端禁止重定向，默认超时 12 秒，单响应最多 4 MiB。所有克隆的客户端共享
同一个请求门，完整网络读取期间只允许一个请求执行，并保证请求起始间隔至少 1 秒。
空成功、超出调用上限、源错误码、非法日期/URL、非有限数或不完整记录都会返回
typed error。

## 字段、单位和证据

- 成交金额、资金流和市值统一为 CNY 元；
- 比率保留 `RatioUnit::Percent`，不会混成 0–1 小数；
- 数量字段按 Core 类型声明的股/手语义输出；
- `source_at` 只取源端明确日期/时间，网页没有可靠批次时间时保持 `None`；
- 每条记录及批次均保留 Provider、源时间、观察时间和批次 ID；
- 人气榜与 Quote 来自两个请求，分别保留 ranking/quote evidence，禁止伪装成原子快照。

## 探针

完整 live probe 会打印所有 capability、provenance、quality 和记录字段：

```bash
cargo run -p magic-eastmoney-rs --example live_probe --release --locked --offline
```

有界 load probe 支持 `research`、`fund-flow`、`board-flow`、`limit-pool`、
`popularity`、`news` 和 `mixed`：

```bash
MAGIC_EASTMONEY_LOAD_REQUESTS=6 \
MAGIC_EASTMONEY_LOAD_CONCURRENCY=1 \
MAGIC_EASTMONEY_LOAD_PACING_MS=1000 \
cargo run -p magic-eastmoney-rs --example load_probe --release --locked --offline
```

高层数据族 attempt 硬上限为 20（一个 attempt 可能包含多个 HTTP 请求），并发必须
为 1，间隔不得小于 1 秒。`mixed` 只轮转已声明能力；显式选择 `fund-flow` 或
`news` 会输出 `admitted=false`，诊断失败时非零退出，不能将部分成功解释为整个
Provider 已验收。

## 生产边界

这些网页端点没有本项目可证明的版本合同、SLA 或再分发许可，只适合作为盘后研究、
回补和交叉验证源。生产应用需要自行处理授权、调度、缓存、熔断、持久化和使用条款；
本 crate 不提供后台轮询、隐藏重试、跨源拼接或模拟数据回退。
