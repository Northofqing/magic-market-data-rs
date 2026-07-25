# 东方财富公开网页数据接入

`magic-eastmoney-rs` 是与 Choice/EMQuant 完全分离的只读公开网页 Provider。它不读取
东方财富桌面客户端、Cookie、账号、设备激活信息或交易数据，也不会借用
`magic-emquant-rs` 的登录态。

## 已实现的数据族

| 数据族 | 标准化入口 | 已实现范围 |
| --- | --- | --- |
| 研报 | `ResearchReports`、`ResearchDocuments` | 个股、行业研报元数据，以及按精确研报身份下载原始 PDF 正文 |
| 个股资金流 | `FundFlowSeries` | 分钟与日级主力/超大/大/中/小单净流入 |
| 板块资金流 | `BoardFlows` | 行业、概念、地域的涨跌、分档净流入和领涨股 |
| 龙虎榜 | `DragonTigerData` | 个股上榜明细；席位保守返回一个完整买五/卖五原子组，席位请求 `limit >= 10` |
| 全市场龙虎榜 | `DragonTigerDiscovery` | 指定交易日完整读取沪深京股票/可转债；每条保留代码、名称和源 `TRADE_ID` |
| 资本数据 | `MarginData`、`BlockTrades`、`HolderCounts`、`LockupEvents`、`DividendPlans` | 融资融券、大宗交易、股东户数、限售解禁、分红送转 |
| 打板 | `LimitPools` | 涨停、炸板、跌停、昨日涨停 |
| 热度 | `PopularityData` | 当前人气排名，并保留榜单与行情的两份证据 |
| 严格盘后资金榜 | `PostCloseFlows` | 中国当前交易日 15:35 后，精确 limit、同一源时间、连续排名、代码+名称 |
| 最新财经资讯 | `NewsProvider::global_news` | 东财财经滚动页首屏，最多 20 条；完整列表校验后截断 |
| 关键词新闻诊断 | `NewsProvider::instrument_news` | 响应无结构化证券身份，capability 为 false 且正式调用返回 `Unsupported` |

未实现的能力不会由相近字段推测；涨停原因 capability 当前仍为 false。

## 网络与安全边界

实现只允许 HTTPS 443，并限制到以下东方财富主机：

```text
reportapi.eastmoney.com
push2.eastmoney.com
push2his.eastmoney.com
push2ex.eastmoney.com
datacenter-web.eastmoney.com
emappdata.eastmoney.com
pdf.dfcfw.com
roll.eastmoney.com
```

研报正文只从记录绑定的精确 `pdf.dfcfw.com` HTTPS URL 下载；必须返回
`application/pdf`、以 `%PDF-` 开头并且不超过 32 MiB。禁止跳转、HTML、身份错配
和任意 PDF URL。

最新资讯只访问精确的
`https://roll.eastmoney.com/finance.html`，禁止翻页、查询参数和跳转；响应必须是
带 UTF-8 charset 的 `text/html` 且不超过 2 MiB。完整 `#artList` 中每条都必须为
`财经` 分类、分钟时间倒序、标题内外一致，并使用
`finance.eastmoney.com/a/<纯数字 ID>.html`。页面没有证券身份，故
`NewsItem::instruments` 为空，不得转成个股新闻。

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
- 全市场龙虎榜在 limit/交易所过滤前验证完整日数据，股票记录必须同时有代码和名称；
- 15:35 资金榜只接受中国当前日期、捕获时间不早于 15:35、所有行 `f124` 完全一致，
  按 `f62` 非递增且 rank 连续；每条保留 `f14` 名称和 `f184` 主力净占比。

## 探针

完整 live probe 会打印所有 capability、provenance、quality 和记录字段：

```bash
cargo run -p magic-eastmoney-rs --example live_probe --release --locked --offline
```

有界 load probe 支持 `research`、`fund-flow`、`board-flow`、`limit-pool`、
`popularity`、`news` 和 `mixed`；其中 `news` 是已准入的全局最新资讯：

```bash
MAGIC_EASTMONEY_LOAD_REQUESTS=6 \
MAGIC_EASTMONEY_LOAD_CONCURRENCY=1 \
MAGIC_EASTMONEY_LOAD_PACING_MS=1000 \
cargo run -p magic-eastmoney-rs --example load_probe --release --locked --offline
```

高层数据族 attempt 硬上限为 20（一个 attempt 可能包含多个 HTTP 请求），并发必须
为 1，间隔不得小于 1 秒。`mixed` 只轮转已声明能力并包含 `news`；仅
`fund-flow` 会输出 `admitted=false`，诊断失败时非零退出，不能将部分成功解释为
整个 Provider 已验收。

只测试东财最新资讯：

```bash
MAGIC_EASTMONEY_LIVE_OPERATION=global-news \
cargo run -p magic-eastmoney-rs --example live_probe --release --locked --offline
```

## 生产边界

这些网页端点没有本项目可证明的版本合同、SLA 或再分发许可，只适合作为盘后研究、
回补和交叉验证源。生产应用需要自行处理授权、调度、缓存、熔断、持久化和使用条款；
本 crate 不提供后台轮询、隐藏重试、跨源拼接或模拟数据回退。
