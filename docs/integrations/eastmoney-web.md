# 东方财富公开网页数据接入

`magic-eastmoney-rs` 是与 Choice/EMQuant 完全分离的只读公开网页 Provider。它不读取
东方财富桌面客户端、Cookie、账号、设备激活信息或交易数据，也不会借用
`magic-emquant-rs` 的登录态。

## 已实现的数据族

| 数据族 | 标准化入口 | 已实现范围 |
| --- | --- | --- |
| 研报 | `ResearchReports`、`ResearchDocuments` | 个股、行业研报元数据，以及按精确研报身份下载原始 PDF 正文 |
| 个股资金流 | `FundFlowSeries`、gRPC `MoneyFlows` | 生产准入的分钟与日级主力/超大/大/中/小单净流入；`MoneyFlows` 返回单标的最新完整日记录 |
| 板块资金流 | `BoardFlows` | 行业、概念、地域的涨跌、分档净流入和领涨股 |
| 龙虎榜 | `DragonTigerData` | 个股上榜明细；席位保守返回一个完整买五/卖五原子组，席位请求 `limit >= 10` |
| 全市场龙虎榜 | `DragonTigerDiscovery` | 指定交易日完整读取沪深京股票/可转债；每条保留代码、名称和源 `TRADE_ID` |
| 全市场龙虎榜席位 | `MarketDragonTigerData` | 按显式交易日发现全市场上榜项，并按源 `TRADE_ID` 返回每项完整买五/卖五原子组 |
| 资本数据 | `MarginData`、`BlockTrades`、`HolderCounts`、`LockupEvents`、`DividendPlans` | 融资融券、大宗交易、股东户数、限售解禁、分红送转 |
| 打板 | `LimitPools` | 涨停、炸板、跌停、昨日涨停 |
| 热度 | `PopularityData` | 当前人气排名，并保留榜单与行情的两份证据 |
| 盘后资金榜 | `PostCloseFlows` | 中国当前交易日 15:35 后，精确 limit、连续排名、代码+名称和逐条真实源时间；批次使用当前本地观察时间，混合源时刻时批次 `source_at=None` |
| Provider Top-N 排名 | `ProviderTopNRankings` + `EastmoneyProviderTopNRankingRouter` | 同日 15:35 后或后续休市日读取最新已结算交易日的单响应页量比/主力净流入 Top-N；上限 100，每行 `f297` 必须严格等于请求交易日；不声明任意历史、全市场覆盖或 `source_at` |
| 有界市场排名快照 | gRPC `MarketRankings` | 一次 `clist/get` HTTPS 响应中的量比或主力净流入 Top-N；上限 100，严格保留首屏实际行数、源声明总数、代码、名称、指标、单位、连续排名和每行 `f124` 时间；不声明全市场分页完成 |
| 最新财经资讯 | `NewsProvider::global_news` | 东财财经滚动页首屏，最多 20 条；完整列表校验后截断；逐条保留原始分钟 source_at |
| 关键词新闻诊断 | `NewsProvider::instrument_news` | 响应无结构化证券身份，capability 为 false 且正式调用返回 `Unsupported` |

未实现的能力不会由相近字段推测；涨停原因 capability 当前仍为 false。

## 网络与安全边界

实现只允许 HTTPS 443，并限制到以下东方财富主机：

```text
reportapi.eastmoney.com
push2.eastmoney.com
push2delay.eastmoney.com
push2his.eastmoney.com
push2ex.eastmoney.com
datacenter-web.eastmoney.com
emappdata.eastmoney.com
pdf.dfcfw.com
roll.eastmoney.com
stock.eastmoney.com
fund.eastmoney.com
futures.eastmoney.com
bond.eastmoney.com
hk.eastmoney.com
```

`stock.eastmoney.com`、`fund.eastmoney.com`、`futures.eastmoney.com`、`bond.eastmoney.com` 与
`hk.eastmoney.com` 仅允许作为
滚动页返回记录的精确 canonical URL 元数据；适配器
不会向该主机发起第二次请求，也没有新增 Provider-local HTTP/TLS 依赖。

研报正文只从记录绑定的精确 `pdf.dfcfw.com` HTTPS URL 下载；必须返回
`application/pdf`、以 `%PDF-` 开头并且不超过 32 MiB。禁止跳转、HTML、身份错配
和任意 PDF URL。

最新资讯只访问精确的
`https://roll.eastmoney.com/finance.html`，禁止翻页、查询参数和跳转；响应必须是
带 UTF-8 charset 的 `text/html` 且不超过 2 MiB。完整 `#artList` 中每条都必须为
`财经` 分类、分钟时间倒序、标题内外一致，并使用
`finance.eastmoney.com`、`global.eastmoney.com`、`biz.eastmoney.com`、
`stock.eastmoney.com`、`fund.eastmoney.com`、`futures.eastmoney.com`、`bond.eastmoney.com` 或
`hk.eastmoney.com` 上的
`/a/<纯数字 ID>.html`。这些地址只作为来源元数据保存，不会抓取文章正文。页面没有证券身份，故
`NewsItem::instruments` 为空，不得转成个股新闻。

HTTP 客户端禁止重定向，默认超时 12 秒，单响应最多 4 MiB。所有克隆的客户端共享
同一个请求门，完整网络读取期间只允许一个请求执行，并保证请求起始间隔至少 1 秒。
空成功、超出调用上限、源错误码、非法日期/URL、非有限数或不完整记录都会返回
typed error。

## 字段、单位和证据

GlobalNews 的 `published_at` 规范化为 RFC3339，逐条 evidence 和批次首条时间保留
Provider 原始 `YYYY-MM-DD HH:MM`。曾导致持续 `invalid_evidence` 的真实漂移是官方
滚动页开始依次返回 `stock.eastmoney.com`、`fund.eastmoney.com`、`futures.eastmoney.com`、
`bond.eastmoney.com` 和 `hk.eastmoney.com` 文章元数据；当前只对这些精确 HTTPS 主机和既有纯数字文章路径
放行，不借用其他新闻源补齐。

- 成交金额、资金流和市值统一为 CNY 元；
- 比率保留 `RatioUnit::Percent`，不会混成 0–1 小数；
- 数量字段按 Core 类型声明的股/手语义输出；
- `source_at` 只取源端明确日期/时间，网页没有可靠批次时间时保持 `None`；
- 板块资金流必须返回正整数 Unix 更新时间字段 `f124`，且同一原子批次
  的全部记录必须一致；缺失、零值或批次内不一致都会拒绝整个批次；
- 每条记录及批次均保留 Provider、源时间、观察时间和批次 ID；
- 人气榜与 Quote 来自两个请求，分别保留 ranking/quote evidence，禁止伪装成原子快照。
- 龙虎榜买入/卖出总额必须非负；买、卖、净额同时存在时必须满足
  `净额 = 买入 - 卖出`。全市场发现以 `TRADE_ID` 区分同股同日不同上榜原因；
  等价重复稳定保留首条，身份相同但内容冲突会拒绝整批。席位请求同时过滤证券、
  日期和 `TRADE_ID`，每项必须恰有买五和卖五，禁止跨原因混组。
- 全市场龙虎榜在 limit/交易所过滤前验证完整日数据，股票记录必须同时有代码和名称；
- 15:35 资金榜只接受中国当前日期、捕获时间不早于 15:35，按 `f62` 非递增且 rank
  连续；每条保留 `f14` 名称、`f184` 主力净占比和真实 `f124` 源时间。不同证券的
  `f124` 可以不同，但必须同属请求日期；批次使用当前本地 `observed_at`，只在全部源
  时间一致时设置 batch `source_at`，否则保持 `None`。该路径不是 BR-033 实时新鲜度
  合同，也不声明完整市场覆盖。
- Provider Top-N 是独立能力族，只接受同一次 `clist/get` 响应中按 `f10` 或
  `f62` 非递增排列的最多 100 行。每行必须有代码、名称、请求指标和等于请求日的
  `f297`；批次保留响应后的 `observed_at`，但 `f297`/`f124` 都不得提升为
  `source_at`。它不证明完整市场、宽度、覆盖率或截止位并列集合。生产只能通过
  `magic-market-composition::EastmoneyProviderTopNRankingRouter` 的无参数
  生产构造器创建；下游不能注入 transport 或注册本地 wrapper 冒充 Eastmoney。
  请求日不得晚于当前上海日期；同日采集必须在 15:35 后，后续休市日可在任意时刻
  采集，但只有响应中全部 `f297` 仍严格等于请求日才可证明最新已结算会话。该路径
  不提供任意历史回放，旧日期会因 `f297` 不匹配而整批失败。
- gRPC `MarketRankings` 是另一条窄合同。它固定读取第一响应页，要求源返回的实际
  行数精确等于 `min(reported_universe_size, 100)`，再按调用方 `limit <= 100`
  选择前 N 行。每个入选行必须具有 `f12` 代码、`f13` 市场、非空 `f14` 名称、
  请求指标、正确单位和正整数 `f124` 源时间；身份必须唯一、排名连续且指标保持源端
  非递增顺序。任一必填字段缺失或矛盾都会拒绝整个响应。批次证据来自同一 HTTPS
  响应并使用本地 `observed_at`，每行保留自己的 `source_at`；不同证券的时间无需
  相同，也不会合成为一个批次源时刻。这一操作证明的是源端当次 Top-N 快照，不证明
  完整市场分页、覆盖率、截止位并列完整性或可用于市场宽度计算。

2026-08-17 对固定公开资金流合同完成两次独立 live 和三次串行负载验证。
两次 live 均返回 `600396.SH` 的 5 条一分钟记录与 5 条日记录，记录级
`source_at` 分别规范化为带 `+08:00` 的分钟瞬时和 ISO 源日期，所有金额保持
CNY 元。三次负载请求均成功，实际最小请求起始间隔 1000 ms、最大并发 1。
因此 `PUBLIC_FUND_FLOW_ADMITTED=true`，gRPC `FundFlowSeries` 与 `MoneyFlows`
默认使用公开 Eastmoney Provider；配置妙想 Key 不会替换这条正式路径。

2026-08-27 复验发现旧 `push2his` `daykline` 路径持续提前终止 TLS，而主
`push2.eastmoney.com` 在 Rust 严格传输中会返回数据后缺少 TLS `close_notify`。同一 Provider
已登记的官方 `push2delay.eastmoney.com` 当前 `kline/get` 路径使用 `klt=101` 返回日级源日期
以及主力、超大、大、中、小五档净流入。因此生产 `MoneyFlows` 的单条日记录固定使用该精确
官方 delay 主机；分钟记录仍使用主 `push2` 主机。实现不会跨 Provider 补值、不会把 Quote/
成交额推算为资金流，也不会放宽 TLS 校验。确定性测试分别锁定两种 interval 的精确主机、
路径和 `klt`，响应仍需通过证券 identity、日期和逐字段解析校验。

2026-08-16 诊断观察到公开排名响应中部分行缺少量比 `f10` 或主力净流入 `f62`，
因此完整 `MarketRankings` 仍原子失败。gRPC 的显式 UNADMITTED 诊断可返回有字段的
有界行，但会保留各行不同的 `f124` 时间；这仍不会提升完整 `MarketRankings`。

2026-08-18 的契约收窄不再把 gRPC `MarketRankings` 解释为完整多页市场排名：
正式合同只验收一次 HTTPS 响应内的有界 Top-N 快照。旧的多页 Core
`MarketRankings` capability 继续为 `false`。

正式生产探针使用同一命令连续完成 5 轮，之后又完成 1 轮独立成功验证。前 2 轮计为
live、后 3 轮计为串行 load；每轮均从一个 Eastmoney HTTPS 响应返回 20 条记录，代码、
名称、指标、单位、连续排名、逐条源时间、源声明总数和批次证据完整一致。额外成功轮
只作为支持证据，不抬高注册表计数。因此
`BOUNDED_MARKET_RANKINGS_ADMITTED=true`，登记为 2 live + 3 serial。预开盘空指标、
限流响应和旧诊断结果均未计入准入数字。

2026-08-17 对正式 `PostCloseFlows` 完成两次 live 和三次串行 load。live 分别返回
20 条和 3 条，load 3/3 成功；每批质量完整、rank 连续、证券身份唯一、逐条源日期与
请求日相同，实际请求起始间隔最小 1000 ms、最大并发 1。因此
`PUBLIC_POST_CLOSE_FLOW_ADMITTED=true`。混合证券源时刻不会被伪造成公共 batch
`source_at`。

## 探针

完整 live probe 会打印所有 capability、provenance、quality 和记录字段。每个已声明
数据族还必须通过公共 admission verifier：非空、质量完整、记录/批次证据一致、
源时间不晚于观察时间且业务身份唯一。最终成功标记为
`live_probe_status=admitted`；未声明诊断只能输出
`diagnostic_complete_unadmitted` 或 `failed`，不能冒充能力成功：

```bash
MAGIC_EASTMONEY_POOL_DATE=2026-07-24 \
MAGIC_EASTMONEY_DRAGON_TIGER_DATE=2026-07-24 \
cargo run -p magic-eastmoney-rs --example live_probe --release --locked --offline
```

`MAGIC_EASTMONEY_POOL_DATE` 必须由操作者或调度器提供，表示本次验证的源交易
会话；缺失、空值或非法 ISO 日期会显式失败。探针不会用系统日期、工作日猜测或
仓库内硬编码日期代替源会话。`MAGIC_EASTMONEY_DRAGON_TIGER_DATE` 同样必须显式
提供，用于全市场龙虎榜发现与每个 `TRADE_ID` 的 5+5 席位原子性验证。需要只验证
该链路时可运行：

```bash
MAGIC_EASTMONEY_DRAGON_TIGER_DATE=2026-07-22 \
MAGIC_EASTMONEY_DRAGON_TIGER_LIMIT=5 \
cargo run -p magic-eastmoney-rs --example market_dragon_tiger_probe --release --locked
```

只验证当日盘后 Provider Top-N：

```bash
MAGIC_EASTMONEY_LIVE_OPERATION=provider-topn-rankings \
MAGIC_EASTMONEY_TOPN_DATE=<当前 Asia/Shanghai 日期> \
MAGIC_EASTMONEY_RANKING_KIND=all \
cargo run -p magic-eastmoney-rs --example live_probe --release --locked
```

探针逐指标打印 `acquisition_started_at`、批次/首条记录
`observed_at`、`source_at=None`、源总数和检查行数；任一指标失败均非零退出。

独立探针只打印标准化证券身份、源 `entry_id`、席位数量、净买额和批次证据，不输出
原始响应。手动 GitHub Actions 工作流也将源交易日期设为必填输入。
有界 load probe 支持 `research`、`fund-flow`、`post-close-ranking`、`board-flow`、
`limit-pool`、`popularity`、`news` 和 `suite`；其中 `news` 是已准入的全局最新资讯：

```bash
MAGIC_EASTMONEY_LOAD_REQUESTS=6 \
MAGIC_EASTMONEY_LOAD_CONCURRENCY=1 \
MAGIC_EASTMONEY_LOAD_PACING_MS=1000 \
cargo run -p magic-eastmoney-rs --example load_probe --release --locked --offline
```

高层数据族 attempt 硬上限为 21（一个 attempt 可能包含多个 HTTP 请求），并发必须
为 1，间隔不得小于 1 秒。`suite` 只轮转已声明能力并包含 `fund-flow` 与
`post-close-ranking`、`news`；盘后操作还要求显式当前交易日。任一正式能力失败都会
非零退出，不能将部分成功解释为整个 Provider
已验收。

只测试东财最新资讯：

```bash
MAGIC_EASTMONEY_LIVE_OPERATION=global-news \
cargo run -p magic-eastmoney-rs --example live_probe --release --locked --offline
```

## 生产边界

这些网页端点没有本项目可证明的版本合同、SLA 或再分发许可，只适合作为盘后研究、
回补和交叉验证源。生产应用需要自行处理授权、调度、缓存、熔断、持久化和使用条款；
本 crate 不提供后台轮询、隐藏重试、跨源拼接或模拟数据回退。
