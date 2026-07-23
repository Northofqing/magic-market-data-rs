# magic-market-data-rs

面向中国证券市场的 Rust 统一行情工作区。项目把 TDX、腾讯/新浪网页行情和东方财富
Choice/EMQuant 适配到同一组强校验数据契约，并提供保留来源证据的多 Provider
顺序切源能力。

当前代码不固定具体 Rust 版本：开发者使用本机默认工具链，CI 使用当前 stable，
发布制品记录实际 `rustc`/Cargo 版本。生产 Rust 路径禁止 `unsafe`。确定性测试默认
不访问公网；真实行情通过显式运行的只读 probe 验收，不会用 fixture、旧缓存或零值
冒充实盘成功。

> 当前状态（2026-07-23）：TDX、Tencent、Sina、TDX→Tencent 路由、CNInfo、THS、
> CLS 和 Baidu 已通过真实网络验收；Eastmoney 已声明能力的
> live/load 探针全部通过，
> 分钟/日级资金流因当前网络返回 empty reply 而保持未声明能力；关键词新闻响应没有
> 结构化证券身份，也不伪装成个股新闻。两者只作为未准入诊断运行。
> Choice/EMQuant 已完成设备激活和 API 登录，日线与日级资金流已取得
> 真实记录，Quote、盘口和分钟线仍因 `10001012/EQERR_ACCESS_INSUFFICIENCE` 待补
> 权限。iWencai 已实现正式 API Key 鉴权，当前没有授权 Key，因此真实 401 会按设计
> 非零退出，不会伪报成功。

## 项目定位

本仓库提供两类交付物：

- 可被业务服务依赖的 Rust library crates；
- 部署前和故障排查时使用的只读诊断 probe。

它刻意不提供以下能力：

- 常驻行情守护进程、任务调度器或 HTTP/gRPC 服务；
- 数据库、历史数据仓库、跨请求缓存或陈旧数据回填；
- 下单、撤单、持仓、资金、账户或交易登录能力；
- 不可观察的重试、跨 Provider 拼接或模拟数据回退；
- 对公共网页端点的生产 SLA、展示权或再分发授权承诺。

生产应用应在自己的服务层补充并发预算、限频、熔断、缓存、持久化、交易阶段新鲜度
判断和监控。本项目负责源适配、数据校验、证据保留和显式切源。

## 工作区结构

| Crate | 责任 | 明确边界 |
| --- | --- | --- |
| `magic-market-core` | Provider 无关的证券标识、请求、值对象、标准化记录、批次证据和 Provider traits | 不联网，不选择数据源 |
| `magic-market-router` | 第一个合格批次的顺序切源、错误分类、质量门、来源时间门和完整 attempt trace | 不缓存、不跨源合并、不维护熔断状态 |
| `magic-tdx-rs` | 纯 Rust TDX 协议、同步/异步/直连/Smart 客户端、服务门面和本地文件读取器 | MoneyFlow 与集合竞价显式不支持 |
| `magic-tencent-rs` | HTTPS + GBK/JSON 的腾讯补充源，覆盖沪深京基础行情及股票/指数/ETF 行情统计 | 公共网页接口，无正式 SLA |
| `magic-sina-rs` | HTTPS + GB18030/JSON 的新浪补充源，覆盖基础行情、沪深财务三表和沪市 ETF 期权 | 历史分时、逐笔、资金流和竞价不支持；无正式 SLA |
| `magic-emquant-rs` | 通过独立 C++ bridge 调用官方 Choice/EMQuant SDK 的只读适配层 | 厂商 SDK、授权和激活文件不进入仓库 |
| `magic-eastmoney-rs` | 东财公开研报、资金流解析、龙虎榜、资本事件、涨跌停池和人气 | 与 Choice/EMQuant 身份分离；关键词新闻无结构化证券身份，不声明个股新闻能力 |
| `magic-cninfo-rs` | 巨潮公告/PDF 与互动易问答 | 只读公开信息；不读取账户或桌面登录态 |
| `magic-ths-rs` | 同花顺一致预期、强势原因、涨停池和热榜 | 只读公开补充源；字段/频率以当前探针为准 |
| `magic-cls-rs` | 财联社签名电报/全球新闻 | 只支持全局电报，不伪造个股过滤 |
| `magic-baidu-rs` | 百度未复权日线和源端 MA5/10/20 | 不提供 Quote/分钟/Level-2 |
| `magic-iwencai-rs` | 获授权 API Key 的语义搜索 | 无 Key 明确鉴权失败，不复用 Cookie |
| `magic-market-analysis` | 基于标准化记录的均线、估值、涨停情绪和跨源诊断 | 纯函数、不联网；主观估值锚点必须由调用方配置 |

依赖方向保持简单：

```text
业务服务
   │
   ├── magic-market-router ──→ magic-market-core
   │          ▲
   │          └── Provider 注册适配
   │
   ├── magic-tdx-rs ─────────→ magic-market-core
   ├── magic-tencent-rs ─────→ magic-market-core
   ├── magic-sina-rs ─────────→ magic-market-core
   ├── magic-emquant-rs ─────→ magic-market-core
   └── magic-market-analysis → magic-market-core
```

Router 的生产依赖只有 Core，具体 Provider 在应用注册边界接入，避免公共契约反向依赖
某个厂商实现。

## 统一数据契约与证据

Core 当前定义八类基础行情数据族：

| 数据族 | 统一入口 | 关键语义 |
| --- | --- | --- |
| 实时行情 | `RealtimeQuotes` | 当前价、昨收、开高低、量额、名称和状态 |
| K 线 | `HistoricalBars` | 周期、OHLCV、成交额、复权语义和有界数量 |
| 分时 | `MinuteData` | 当日/指定日期分钟点、累计量额与单调性 |
| 逐笔成交 | `Trades` | 成交时间、价格、数量、方向和分页连续性 |
| 资金流 | `MoneyFlows` | 主力及超大/大/中/小单净流入的可审计定义 |
| 五档盘口 | `OrderBooks` | 买卖五档价格/数量、可见总深度和缺档状态 |
| 集合竞价 | `Auctions` | 撮合价、匹配量及未匹配买卖量 |
| 证券元数据 | `SecurityMetadataProvider` | 名称、ST、板块、上市日和涨跌停规则 |

每条标准化记录与批次都保留证据，而不是只返回裸数值：

- `ProviderId`：真实来源，切源后也不改写；
- `source_at`：只有源报文能够证明时才存在；
- `observed_at` 或批次 `fetched_at`：本机观测/抓取时间；Bar 等批次型记录由
  provenance 保存抓取时间；
- `batch_id`：记录与 provenance 必须一致；
- `DataStatus`：`Available`、`Unavailable` 等显式状态；
- `QualityReport`：缺字段、未验证来源时间或部分可用原因。

构造器拒绝非有限数、非法价格/数量、空证据、代码错配、重复/乱序记录、OHLC
矛盾、盘口半档和批次证据不一致。缺失值保持缺失并产生质量证据，不会填 `0`；
不支持的数据族返回 typed error，不会返回空成功批次。

### 扩展数据与分析契约

参考 [a-stock-data](https://github.com/simonlin1212/a-stock-data) 的产品能力分层后，
Core 已增加以下 Provider 无关领域。表中“实盘”表示对应 Provider 已通过真实网络
probe；其余“契约/路由完成”只表示 Rust 类型、受检反序列化、Provider trait 和
Router 适配器已经通过确定性测试。

| 领域 | 主要记录 | 当前状态 |
| --- | --- | --- |
| 行情增强 | `MarketStatistics`、`TechnicalBar` | Tencent 股票/指数/ETF 统计与 Baidu 未复权日 K/MA 实盘 |
| 研报与一致预期 | `ResearchReport`、`ConsensusSnapshot`、`SemanticSearchDocument` | Eastmoney 研报、THS 一致预期实盘；iWencai 已实现/待授权 Key |
| 信号与板块 | `BoardMembership`、`StrongStockReason`、龙虎榜/人气/概念记录 | Eastmoney 龙虎榜/人气与 THS 强势原因/热榜实盘；板块归属/概念命中待源 |
| 资金面与筹码 | `FundFlowPoint`、`BoardFlow`、融资融券、大宗、户数、解禁、分红 | Eastmoney 除资金流 host 当前网络失败外均实盘；资金流解析/fixture 已完成 |
| 盘后资金流排行 | `PostCloseFlow`、`PostCloseFlowRequest` | 契约/路由完成；没有 Provider 获得已验证的 15:35 Top10 语义 |
| 新闻/公告/互动 | `NewsItem`、`Announcement`、`InvestorQuestion` | CLS 全球电报、CNInfo 公告/互动易实盘；个股新闻仍待有结构化证券身份的来源 |
| 公司与财报 | `SecurityProfile`、三类 `FinancialStatement` | Sina 沪深三表实盘；SecurityProfile/TDX 映射待接 |
| 打板 | 四类 `LimitPoolEntry` | Eastmoney 四类池与 THS 涨停池实盘；字段缺失不跨源猜测 |
| ETF 期权 | `OptionContract`、`OptionQuote`、`OptionGreeks` | Sina 510050 实盘；510300/588000/510500 已实现待实测 |

所有扩展记录使用受检 `SourceEvidence`；非空文本、HTTPS URL、Gregorian 日期、
有限数和正排名都无法通过反序列化绕过。人气榜与 Quote、价格与一致预期等非原子
拼接必须保留每个输入的 Provider/批次证据。

`magic-market-analysis` 当前提供：

- 有明确 warm-up 空值的 SMA，拒绝乱序或混合证券/周期；
- Forward PE、PEG 和调用方配置目标 PE 的消化年数，拒绝零/负分母；
- 四类涨跌停池情绪，空分母时 seal rate 保持 `None`；
- 跨 Provider 观测时间和数值离散度，拒绝同一 Provider 重复冒充多源；
- `ProviderId::LocalAnalysis` 派生值及完整输入证据。

## Provider 能力矩阵

状态说明：

- **实盘**：实现已通过真实网络 probe；
- **已实现/待权限**：代码和确定性测试完成，但当前账号/字段尚未实盘验收；
- **部分**：只覆盖表中写明的市场、周期或字段；
- **不支持**：入口显式返回 `Unsupported`。

| 能力 | TDX | Tencent | Sina | Choice/EMQuant |
| --- | --- | --- | --- | --- |
| Quote | 实盘：沪深京 | 实盘：沪深京 | 实盘：沪深京 | 已实现/权限不足：查询返回 `10001012` |
| K 线 | 实盘：个股/指数，12 类周期（1 分钟至年线） | 部分实盘：沪深 1/5/15/30/60 分钟、日/周/月；北京日线 | 部分实盘：1/5/15/30/60 分钟、日线；北京 5 分钟/日线实测 | 部分实盘：日线通过；分钟返回 `10001012`；周/月/年已实现待实测 |
| 分时 | 实盘：当日与按日期历史 | 实盘：当日与历史；市场边界见专项文档 | 部分实盘：最新交易日，由 1 分钟线累计；历史日期不支持 | 未接入独立 `MinuteData`；分钟 K 见上一行 |
| 逐笔 | 实盘：当日与历史，自动翻页 | 部分实盘：沪深当日；历史与北京不支持 | 不支持 | 不支持 |
| 五档盘口 | 实盘：沪深京 | 实盘：沪深京 | 实盘：沪深京；真实空侧标部分不可用 | 已实现/权限不足：查询返回 `10001012` |
| 行情统计 | 不支持统一契约 | 实盘：股票/指数/ETF 的换手、PE/PB、市值、涨跌停价、量比 | 不支持 | 当前未接入 |
| 资金流 | 不支持 | 不支持 | 不支持 | 实盘：日级大中小单净流入 |
| 证券列表/元数据 | 沪深全市场列表与部分标准化元数据；北京列表端点不支持 | 部分：名称/ST、派生板块；缺上市日和规则版本 | 部分：名称/ST、派生板块；缺上市日和规则版本 | 未验证，当前 capability 关闭 |
| 财务数据 | 实盘：实时 34 项、报告包和 45 个命名指标 | 不支持 | 实盘：沪深资产负债表/利润表/现金流量表，各最近 8 期 | 当前未接入统一财务契约 |
| ETF 期权 | 不支持 | 不支持 | 510050 实盘；另 3 个沪市 ETF 已实现待实测 | 不支持 |
| 除权除息 | 实盘：XDXR 分红/送股/配股/缩股历史 | 不支持 | 不支持 | 当前未接入 |
| 板块/F10/基金 | 实盘：行业/概念/指数、F10、基金数据 | 不支持 | 不支持 | 当前未接入 |
| 开盘集合竞价 | 不支持 | 不支持 | 不支持 | 不支持：完整字段集尚未证明 |

### 公共研究、内容与信号 Provider

| Provider | 已真实取得 | 当前明确边界 |
| --- | --- | --- |
| Eastmoney Web | 个股/行业研报、三类板块流、龙虎榜、融资融券、大宗、户数、解禁、分红、四类打板、人气 | 当前网络对两个资金流 host 返回 empty reply；关键词新闻无证券身份；PDF 只给 URL；无已验证 15:35 Top10 |
| CNInfo | 华电辽能公告/PDF metadata、比亚迪互动易问答 | 内容源，不提供行情；PDF 不由 crate 下载 |
| THS | 一致预期、强势原因、涨停池/原因、股票热榜 | 只声明已验证涨停池，不声明其他三类池 |
| CLS | 签名全球电报及来源时间、发布者、关联股票/主题 | 不伪造个股过滤，不是行情源 |
| Baidu | 华电辽能未复权日 K、MA5/10/20 | 不提供实时 Quote、分钟线或 Level-2 |
| iWencai | 正式 X-Claw 鉴权和语义结果解析 | 真实数据待合法 API Key；不读取 Cookie/桌面登录态 |

### TDX

TDX 是当前覆盖最广的纯 Rust 数据源，包含 Blocking、Direct、Tokio Async 和
Smart failover 客户端。2026-07-23 的真实 probe 覆盖：

- 华电辽能 `600396.SH`、平安银行 `000001.SZ`、太湖远大 `920118.BJ`；
- 12 类个股 K 线和指数 K 线；
- 五档、当日/历史分时、当日/历史逐笔及跨页大样本；
- 沪深证券列表、实时财务、财务报告包、45 个英文指标、XDXR；
- 行业/概念/指数板块、基金和 F10。

TDX Quote/盘口报文中的时间区域格式尚未完成审计，所以标准化记录保留
`source_at=None` 并标记质量不完整。它们不能直接通过要求可审计源时间的 5 秒新鲜度
门。TDX 也没有满足统一 MoneyFlow 和集合竞价契约的字段，不会从成交或盘口推测。

完整边界见 [TDX 能力矩阵](docs/TDX_CAPABILITIES.md)。

### Tencent

Tencent 是基础行情补充源。它不读取桌面客户端、Cookie、账户或设备令牌，只访问
公开网页行情端点。Quote 提供可验证的 `YYYYMMDDHHMMSS` 源时间，但网页接口没有
正式版本合同或 SLA，不能单独承担生产主链路。

同一快照的扩展字段已标准化为 `MarketStatistics`：股票、指数和 ETF 的换手率、
动态/静态 PE、PB、总/流通市值、涨跌停价和量比。市值从源端“亿元”转换为 CNY 元；
空字段保持 `None`，指数 `-1` 涨跌停占位不会冒充价格。

2026-07-23 的真实上限短压测为 100 请求、8 并发、100/100 成功、3,700 条记录、
56.49 req/s、P50 100.077 ms、P95 219.676 ms、最大 251.169 ms。这只是短时诊断，
不是允许调用频率或厂商性能承诺。

新增统计专项短压测为 12 请求/3 并发、12/12 成功、36 条记录、28.76 req/s、
P50 66.801 ms、P95 181.955 ms、最大 192.500 ms。

市场、周期、单位、盘后分钟点和端点边界见
[Tencent 接入合同](docs/integrations/tencent-web.md)。

### Sina

Sina 是第二个公共网页补充源，只访问 `hq.sinajs.cn`、`quotes.sina.cn` 和
`stock.finance.sina.com.cn`。快照按 GB18030 严格解码，K 线和财务响应按 JSON
严格校验。源端数量是“股”，适配边界统一除以 100 输出“手”；日线官方响应没有
成交额时保持 `None`。

2026-07-23 的真实 probe 覆盖华电辽能、平安银行和太湖远大，验证了三市场 Quote、
五档、1/5/15/30/60 分钟线、日线、北京 5 分钟/日线和最新交易日分时。华电辽能
涨停时卖侧真实为空，盘口正确标记部分不可用。

同一完整 probe 还取得华电辽能资产负债表、利润表和现金流量表各最近 8 期，并打印
每个稳定英文源字段、中文标签、值、币种、报告期和公告日。ETF 期权实盘发现 510050
全部可用月份和认购/认沽合约，并取得两个合约的最优买卖一档 T 型报价、
Delta/Gamma/Theta/Vega、IV 和理论价格；`rho` 因源端不提供保持 `None`。

最终复测的默认 20 请求/4 并发 mixed 短压测 20/20 成功，共 1,477 条记录、
11.69 req/s、P50 207.786 ms、P95 645.489 ms、最大 788.549 ms。这不是 SLA 或
推荐调用频率。

财务专项 6 请求/2 并发为 6/6 成功、48 个报告期、18.19 req/s；期权专项 6 请求/
2 并发为 6/6 成功、24 条报价/Greeks 记录、22.30 req/s。它们同样只是有界诊断。

字段、单位、最新分时推导、显式不支持和部署边界见
[Sina 接入合同](docs/integrations/sina-web.md)。

### Choice/EMQuant

Rust 适配器和只读 snapshot bridge 已实现 Quote、日/周/月/年 K 线、
1/5/15/30/60 分钟线、五档和日级资金流。bridge 作为子进程隔离厂商 C++ ABI，使
Rust workspace 保持无 `unsafe`。

2026-07-23 权限生效后，官方 SDK 已能登录；真实 probe 取得华电辽能、平安银行的
完整日级资金流和华电辽能最近五根不复权日线。SDK 的日线日期是非补零
`YYYY/M/D`，适配层现已严格标准化为 `YYYY-MM-DD`。

同一轮 probe 中，Quote、五档和 `chmc` 分钟线均返回
`10001012/EQERR_ACCESS_INSUFFICIENCE`。本地官方头文件将其定义为账号已认证但
当前服务/字段权限不足；这不是设备激活、动态库、服务器列表或桥接器故障。补充相应
行情/Level-2/分钟历史权限前，完整 EMQuant probe 会保持非零退出，不回退到其他源或
模拟数据。

SDK 安装、签名、激活和错误码说明见
[Choice/EMQuant 接入文档](docs/integrations/eastmoney-emquant.md)。

## 快速开始

### 环境要求

- Git；
- 当前 stable Rust/Cargo，并提供 rustfmt 和 Clippy；
- Bash 和常用 Unix 工具（发布脚本）；
- 首次获取依赖时允许访问 crates.io；
- 运行真实 probe 时允许对应 Provider 的出站网络。

准备 stable 工具链并获取锁定依赖：

```bash
git clone https://github.com/Northofqing/magic-market-data-rs.git
cd magic-market-data-rs
cargo fetch --locked
```

仓库不通过脚本安装或切换工具链。`Cargo.lock` 固定依赖解析结果；不要删除锁文件后
直接升级，否则可能改变编译器要求和传递依赖。

### 确定性验证

依赖已经获取后，以下命令不访问行情公网：

```bash
cargo check --workspace --all-targets --locked --offline
cargo test --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo doc --workspace --no-deps --locked --offline
```

完整发布门使用当前默认工具链，一次执行格式、全目标编译、全部测试、严格 Clippy、
rustdoc、doctest、链接、合规和 diff 检查：

```bash
bash tools/release/preflight.sh
```

## 真实数据探针

所有 live probe 都会打印标准化字段、来源和质量信息，并以退出码表达真假。登录
失败、预期能力缺记录、代码错配、协议矛盾、超时或权限不足都会退出非零；不会打印
fixture 后返回成功。

命令中的 Cargo `--offline` 只禁止 Cargo 重新解析/下载依赖，不会阻止编译出的
probe 访问行情网络。

### TDX 全能力 probe

```bash
cargo run -p magic-tdx-rs --example live_probe --release
```

该命令打印 Quote、全部 K 线周期、五档、分时、逐笔、证券列表/数量、实时与报告期
财务、45 个命名指标、XDXR、板块、基金和 F10。财务报告包会校验 HTTP 边界、ZIP
目录、解压长度和 CRC。

### Tencent 功能 probe

```bash
MAGIC_TENCENT_CODES=600396.SH,000001.SZ,920118.BJ \
MAGIC_TENCENT_STATISTICS_CODES=600396.SH:EQUITY,000001.SH:INDEX,510050.SH:ETF \
MAGIC_TENCENT_HISTORY_DATE=2026-07-22 \
cargo run -p magic-tencent-rs --example live_probe --release --locked --offline
```

默认超时可通过 `MAGIC_TENCENT_TIMEOUT_SECS` 调整。盘前零现价、涨跌停缺档和特定
市场端点空结果会按协议边界失败或标记质量降级，不会被改写成其他市场数据。

### Tencent 有界并发 probe

```bash
MAGIC_TENCENT_LOAD_OPERATION=mixed \
MAGIC_TENCENT_LOAD_REQUESTS=20 \
MAGIC_TENCENT_LOAD_CONCURRENCY=4 \
cargo run -p magic-tencent-rs --example load_probe --release --locked --offline
```

`MAGIC_TENCENT_LOAD_OPERATION` 可选 `quotes`、`bars`、`minute`、`trades`、
`statistics` 或 `mixed`。程序在联网前强制最多 100 请求、8 个线程，防止把诊断
工具误用成无限压测。

### Sina 功能 probe

```bash
MAGIC_SINA_CODES=600396.SH,000001.SZ,920118.BJ \
MAGIC_SINA_OPTION_UNDERLYING=510050 \
MAGIC_SINA_OPTION_SAMPLE_CONTRACTS=2 \
cargo run -p magic-sina-rs --example live_probe --release --locked --offline
```

默认超时 10 秒，可由 `MAGIC_SINA_TIMEOUT_SECS` 调整。probe 打印三市场 Quote、
全部五档、部分元数据、六个支持 K 线周期、北京 5 分钟/日线和每个证券的最新交易日
分时、沪深财务三表、ETF 期权合约、T 型报价和 Greeks/IV；日线成交额缺失、
涨跌停盘口空侧和源端未提供的 rho 保持真实缺失。

### Sina 有界并发 probe

```bash
MAGIC_SINA_LOAD_OPERATION=mixed \
MAGIC_SINA_LOAD_REQUESTS=20 \
MAGIC_SINA_LOAD_CONCURRENCY=4 \
cargo run -p magic-sina-rs --example load_probe --release --locked --offline
```

operation 可选 `quotes`、`bars`、`minute`、`financial`、`options` 或 `mixed`。
`options` 默认在同次运行开始时发现当前合约并选择
`MAGIC_SINA_OPTION_SAMPLE_CONTRACTS` 个样本；也可用
`MAGIC_SINA_OPTION_CONTRACTS` 显式覆盖。程序在联网前强制最多 40 请求、4 个线程。

### TDX→Tencent 路由 probe

```bash
cargo run -p magic-market-router --example live_probe --release --locked --offline
```

probe 要求批次质量完整且存在来源时间。TDX 会返回真实 Quote，但因名称/源时间证据
不足被质量门拒绝；随后 Tencent 返回合格批次并被选中。输出同时保留 TDX 拒绝和
Tencent 选中的 attempt trace。

### Choice/EMQuant probe

先用获授权的官方 macOS SDK 构建本机 bridge：

```bash
bash tools/emquant/check_sdk.sh /approved/path/EMQuantAPI_CPP_Mac
bash tools/emquant/build_snapshot_bridge.sh /approved/path/EMQuantAPI_CPP_Mac
```

若返回 `10001014/EQERR_NEED_ACTIVATE`，运行同级官方激活器并完成短信激活：

```bash
target/emquant/runtime/loginactivator_mac
```

然后运行：

```bash
MAGIC_EMQUANT_CODES=600396.SH,000001.SZ \
cargo run -p magic-emquant-rs --example live_probe --release
```

`MAGIC_EMQUANT_TIMEOUT_SECS` 可覆盖默认 30 秒子进程超时。只有真实 Quote、五档、
资金流、日 K 和分钟 K 全部按预期返回时，probe 才退出零。厂商 SDK、加密服务器
列表、动态库和 `userInfo` 都只存在于 Git 忽略的本机 runtime，不进入 release 包。

### 公开研究、内容与信号 probes

```bash
cargo run -p magic-eastmoney-rs --example live_probe --release --locked --offline
cargo run -p magic-cninfo-rs --example live_probe --release --locked --offline
cargo run -p magic-ths-rs --example live_probe --release --locked --offline
cargo run -p magic-cls-rs --example live_probe --release --locked --offline
cargo run -p magic-baidu-rs --example live_probe --release --locked --offline
```

每个 crate 另有同名 `load_probe`。Eastmoney 最多 20 次高层数据族 attempt
（部分数据族内部包含多个 HTTP 请求），CNInfo/THS 最多 5 请求，
CLS/Baidu 最多 3 请求；这些公共网页 probe 都强制并发 1、请求间隔至少 1 秒。默认
证券和日期可以通过各 crate 文档列出的 `MAGIC_*` 环境变量覆盖。

iWencai 必须使用单独获授权的 API Key：

```bash
MAGIC_IWENCAI_API_KEY=... \
cargo run -p magic-iwencai-rs --example live_probe --release --locked --offline
```

缺少 Key 或真实 HTTP 401/403 会返回脱敏的 typed `Authentication` 错误。程序不会
读取浏览器 Cookie 或复用同花顺桌面客户端登录态。

## 多数据源路由

`magic-market-router` 对每个数据族使用独立的 `FailoverChain`：

```text
同一不可变请求
    │
    ├─ Provider A ─ 终止错误 ───────────────→ 整体失败
    │              可恢复错误/质量拒绝 ─┐
    ├─ Provider B ←──────────────────────┘
    │              合格批次 ─────────────→ 批次 + 完整 attempt trace
    └─ Provider C ─ 全部失败 ─────────────→ Exhausted + 完整 attempt trace
```

Router 永远拒绝空批次、Provider ID 错配、缺失 provenance batch ID 和记录/批次 ID
不一致。调用方还可以要求：

- `require_complete`：拒绝带任何质量问题的批次；
- `require_source_at`：拒绝没有批次级来源时间的批次。

Provider 错误必须在注册点明确映射为 `InvalidRequest`、`Unsupported`、
`Transport`、`Timeout`、`RateLimited`、`NoData`、`Protocol` 或 `Provider`，
同时选择 `Stop` 或 `TryNext`。非法请求必须停止，不能靠后续 Provider 的偶然成功
掩盖调用缺陷。

最小 Tencent Quote 注册示例：

```rust
use magic_market_core::ProviderId;
use magic_market_router::{
    quote_source, AcceptancePolicy, FailureKind, QuoteRouter, SourceError,
};
use magic_tencent_rs::{TencentClient, TencentError};
use std::error::Error;
use std::sync::Arc;

fn build_router() -> Result<QuoteRouter, Box<dyn Error>> {
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
            other => {
                SourceError::try_next(FailureKind::Protocol, other.to_string())
            }
        },
    ))?;
    Ok(router)
}
```

成功时读取 `RouteOutcome::selected_provider()`、`batch()` 和 `attempts()`；失败时从
`RouterError::attempts()` 保留完整诊断。Router 不解析一个适用于所有数据族的固定
“5 秒”规则：Quote、分钟线、日线和盘后指标的时间语义不同，新鲜度门属于业务层。

完整策略见 [多数据源路由文档](docs/MULTI_PROVIDER_ROUTING.md)。

## 构建发布与部署

### 可重复发布

发布前 tracked worktree 必须干净：

```bash
bash tools/release/preflight.sh
git commit
bash tools/release/package.sh
```

打包输出位于 `target/dist/GIT_SHA/`：

```text
target/dist/GIT_SHA/
├── bin/
│   ├── magic-baidu-live-probe
│   ├── magic-baidu-load-probe
│   ├── magic-cls-live-probe
│   ├── magic-cls-load-probe
│   ├── magic-cninfo-live-probe
│   ├── magic-cninfo-load-probe
│   ├── magic-emquant-live-probe
│   ├── magic-eastmoney-live-probe
│   ├── magic-eastmoney-load-probe
│   ├── magic-iwencai-live-probe
│   ├── magic-iwencai-load-probe
│   ├── magic-router-live-probe
│   ├── magic-sina-live-probe
│   ├── magic-sina-load-probe
│   ├── magic-tdx-live-probe
│   ├── magic-tencent-live-probe
│   ├── magic-tencent-load-probe
│   ├── magic-ths-live-probe
│   └── magic-ths-load-probe
├── docs/
├── licenses/
├── Cargo.lock
├── RELEASE_REVISION
├── RUSTC_VERSION
├── CARGO_VERSION
├── TARGET_TRIPLE
└── SHA256SUMS
```

校验当前提交制品：

```bash
release_dir=target/dist/$(git rev-parse HEAD)
cd "$release_dir"
shasum -a 256 -c SHA256SUMS
```

Linux 可用 `sha256sum -c SHA256SUMS`。制品绑定构建它的 OS、CPU 架构、Rust/Cargo
版本和 Git SHA，不能把 macOS 二进制当成 Linux/Windows 制品。

### 平台与网络摘要

| 组件 | 平台 | 必需运行时访问 |
| --- | --- | --- |
| Core / Router | macOS、Linux、Windows | 无 |
| TDX | macOS、Linux、Windows | 行情服务器 TCP 7709；财务包 `data.tdx.com.cn:80` |
| Tencent | macOS、Linux、Windows | `qt.gtimg.cn`、`web.ifzq.gtimg.cn`、`ifzq.gtimg.cn`、`stock.gtimg.cn` 的 HTTPS |
| Sina | macOS、Linux、Windows | `hq.sinajs.cn`、`quotes.sina.cn`、`stock.finance.sina.com.cn` 的 HTTPS |
| Eastmoney Web | macOS、Linux、Windows | `reportapi`、`push2/push2his/push2ex`、`datacenter-web`、`emappdata` 等文档列出的 HTTPS 主机 |
| CNInfo | macOS、Linux、Windows | `www.cninfo.com.cn`、`irm.cninfo.com.cn`、`static.cninfo.com.cn` 的 HTTPS |
| THS | macOS、Linux、Windows | `basic`、`zx`、`data`、`dq.10jqka.com.cn` 的 HTTPS |
| CLS | macOS、Linux、Windows | `www.cls.cn` 的 HTTPS |
| Baidu | macOS、Linux、Windows | `finance.pae.baidu.com` 的 HTTPS |
| iWencai | macOS、Linux、Windows | `openapi.iwencai.com` 的 HTTPS；需要获授权 API Key |
| 当前 EMQuant bridge | x86_64 macOS | 厂商加密服务器列表定义的目标；本机官方 SDK |

TDX SmartClient 需要服务账号拥有独立可写目录来保存服务器健康缓存。TDX 财务包
沿用厂商 HTTP 分发端点，代码校验结构、长度和 CRC，但传输层不加密；严格环境应
关闭该能力或接入经过批准的完整性代理。

当前 EMQuant C++ bridge 使用 macOS `.dylib`、`dlopen` 和 POSIX API。Linux 或
Windows 虽可编译 Rust 层，但运行前必须基于对应平台官方 SDK 单独实现并验收。
Apple Silicon 只有 x86_64 SDK 时，整条 EMQuant 进程链必须在 x86_64/Rosetta
环境运行，不能跨架构加载动态库。

完整文件布局、健康检查、容器要求、回滚和升级流程见
[部署手册](docs/DEPLOYMENT.md)。

### 生产集成责任

业务守护进程应长期复用 Provider client，而不是为每条记录启动 probe，并负责：

1. Provider 级并发上限、请求超时、退避和熔断；
2. 交易阶段感知的新鲜度门；
3. 缓存与数据库写入时保留真实来源和质量证据；
4. 监控延迟、空结果、质量降级、源时间倒退和切源次数；
5. 优雅停机和在途请求收敛；
6. 按数据供应商协议控制展示、再分发和调用频率。

## 安全与合规边界

- 全部 Provider 接入均为只读市场数据；项目不访问账户、持仓、资金或下单接口。
- 不代理、解密、重放东方财富桌面客户端或其他终端的私有登录流量。
- 用户名、密码、手机号、验证码、Cookie、设备令牌、`userInfo` 和登录报文不得进入
  源码、fixture、日志、镜像或 release 包。
- EMQuant 厂商动态库、加密服务器列表和图片资源受厂商许可证约束，只能在获授权
  主机本地准备。
- Tencent、Sina、Eastmoney、CNInfo、THS、CLS 和 Baidu 公共网页端点没有本项目
  可证明的 SLA 或再分发许可，部署方必须自行确认服务条款。
- 未验证字段必须保持 `None`/`Unavailable` 或返回 `Unsupported`，不得通过猜测、
  跨源填补或模拟记录“修好”。
- Probe 输出可记录 Provider、证券代码、批次 ID、质量问题、耗时和错误码，但不得
  输出任何激活令牌或个人信息。

## 当前验收状态

以下是 2026-07-23 已保存的验收边界，不等同于供应商 SLA：

| 项目 | 结果 | 证据摘要 |
| --- | --- | --- |
| 默认工具链全工作区门禁 | 通过 | 实际版本由 preflight 输出；check、全部测试、严格 Clippy、rustdoc/doctest、链接和合规 |
| TDX live probe | 通过 | 沪深京基础行情、12 K 线周期、分时/逐笔、财务/XDXR、板块/基金/F10 |
| Tencent live probe | 通过 | 沪深京基础行情；股票/指数/ETF 行情统计；沪深当日逐笔 |
| Tencent load probe | 通过 | mixed 100/8 为 100/100；统计 12/3 为 12/12 |
| Sina live probe | 通过 | 基础行情、三类财务报表各 8 期、510050 合约/T 型报价/Greeks/IV |
| Sina load probe | 通过 | mixed 20/4、财务 6/2、510050 期权 6/2 均零失败 |
| Router live probe | 通过 | TDX 质量拒绝被保留，Tencent 合格 Quote 被选中 |
| EMQuant live probe | 部分通过 | 登录成功；真实日线、日级资金流通过；Quote/盘口/分钟返回 `10001012`，完整 probe 按设计退出非零 |
| Eastmoney live/load | 通过（已声明能力） | live 的研报、板块、龙虎榜、资本数据、四类池和人气通过；load 3/3、最小高层 attempt 起始间隔 1002 ms；未声明资金流/关键词新闻保持诊断 |
| CNInfo live/load | 通过 | 公告 3 条、互动易 3 条；load 3/3，最小请求起始间隔 1004 ms |
| THS live/load | 通过 | 一致预期、强势原因、涨停池、热榜；load 3/3，最小请求起始间隔 1002 ms |
| CLS live/load | 通过 | 签名电报 5 条；load 2/2、20 条记录、零失败 |
| Baidu live/load | 通过 | 华电辽能未复权日 K/MA；load 2/2、40 条记录、零失败 |
| iWencai live | 待授权 | 无 Key 的真实 HTTP 401 正确映射为脱敏鉴权错误；未伪造数据 |
| Release package | 每个提交独立构建 | 十九个独立 probe、跟踪文档、许可证、构建元数据和 SHA-256 清单 |

任何 Provider 字段、授权、服务器或网页协议发生变化后，都必须重新运行对应的
确定性测试和真实 probe。旧验收记录不能自动证明新版本仍然可用。

## 文档索引

| 文档 | 内容 |
| --- | --- |
| [部署手册](docs/DEPLOYMENT.md) | 可重复构建、平台、网络、EMQuant runtime、健康检查、回滚与升级 |
| [TDX 能力矩阵](docs/TDX_CAPABILITIES.md) | 全数据族、北京市场、证据和显式不支持边界 |
| [Tencent 接入合同](docs/integrations/tencent-web.md) | 端点、统计字段/单位、市场/周期边界与负载结果 |
| [Sina 接入合同](docs/integrations/sina-web.md) | 基础行情、财务三表、ETF 期权、字段与负载结果 |
| [Choice/EMQuant 接入](docs/integrations/eastmoney-emquant.md) | SDK bridge、激活、能力映射和当前权限状态 |
| [Eastmoney Web 接入](docs/integrations/eastmoney-web.md) | 研报、资金面、龙虎榜、打板、人气及未准入诊断 |
| [CNInfo 接入](docs/integrations/cninfo-web.md) | 证券/org 映射、公告/PDF 和互动易问答 |
| [THS 接入](docs/integrations/tonghuashun-web.md) | 一致预期、强势原因、涨停池和热榜 |
| [CLS 接入](docs/integrations/cls-web.md) | 签名全球电报、字段和限流边界 |
| [Baidu 接入](docs/integrations/baidu-web.md) | 未复权日 K 与源端 MA5/10/20 |
| [iWencai 接入](docs/integrations/iwencai-api.md) | API Key 鉴权、语义搜索和脱敏错误 |
| [多数据源路由](docs/MULTI_PROVIDER_ROUTING.md) | 错误分类、接受政策、attempt trace 和真实切源 |
| [性能结果](docs/PERFORMANCE_RESULTS.md) | 可复现性能证据及适用范围 |
| [业务规则](docs/business_rules.md) | Smart server、重试和服务行为规则 |
| [工程规则](docs/ENGINEERING_RULES.md) | 不变量、测试、错误和发布要求 |
| [上游说明](docs/UPSTREAM.md) | TDX 来源代码和维护边界 |
| [变更记录](CHANGELOG.md) | 未发布版本的破坏性迁移和新增能力 |

## 上游与许可证

工作区自身使用 `MIT OR Apache-2.0` 双许可证，详见
[LICENSE-MIT](LICENSE-MIT) 和 [LICENSE-APACHE](LICENSE-APACHE)。

`magic-tdx-rs` 起源于 MIT 许可的 `tdxrs` 代码，随后直接纳入本工作区并围绕强校验
数据契约、服务门面、并发客户端、真实探针和发布门禁进行了扩展。上游 MIT 文本保留
在 [LICENSES/tdxrs-MIT.txt](LICENSES/tdxrs-MIT.txt)，详细来源与差异见
[docs/UPSTREAM.md](docs/UPSTREAM.md)。

Choice/EMQuant 厂商 SDK 不属于仓库开源制品；所有第三方网络数据的使用、展示和
再分发仍受各自供应商条款约束。
