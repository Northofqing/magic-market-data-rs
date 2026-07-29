# magic-market-data-rs

面向中国证券市场的 Rust 统一行情工作区。项目把 TDX、腾讯/新浪网页行情和东方财富
Choice/EMQuant 适配到同一组强校验数据契约，并提供保留来源证据的多 Provider
顺序切源能力。

当前代码不固定具体 Rust 版本：开发者使用本机默认工具链，CI 使用当前 stable，
发布制品记录实际 `rustc`/Cargo 版本。生产 Rust 路径禁止 `unsafe`。确定性测试默认
不访问公网；真实行情通过显式运行的只读 probe 验收，不会用 fixture、旧缓存或零值
冒充实盘成功。

> 当前状态（2026-07-29）：TDX、Tencent、Sina、TDX→Tencent 路由、CNInfo、THS、
> CLS、Jin10、The Paper、Baidu，以及 SSE/SZSE 官方公告与龙虎榜、SZSE Quote/五档和 HKEX
> 北向日统计已通过真实网络验收；CFFEX 官方 IF/IH/IC/IM 交割通知的确定性解析已实现，
> 但 2026-07-27 的 Rustls 与 Native TLS 实网均在取得 HTTP 前握手失败，所以正式
> capability 仍为 `false`，不会用明文 HTTP 或日期公式绕过生产门禁。
> Eastmoney 已声明能力的 live/load 探针全部通过，
> 分钟/日级资金流因当前网络返回 empty reply 而保持未声明能力；关键词新闻响应没有
> 结构化证券身份，也不伪装成个股新闻。东财财经滚动页已作为独立全局最新资讯实现，
> 不把新闻文本提升为证券身份。
> 韩联社简体中文 7 路 RSS 的 metadata-only Provider、严格解析和探针已实现；2026-07-26
> release Rust 客户端在沙箱内外访问 Rolling/Economy 均收到 TLS unexpected EOF，
> 因此 `global_news=false`、生产 trait 返回 `Unsupported`，只保留显式诊断入口。
> 华尔街见闻单一公开 RSS 的 metadata-only Provider 已通过 release live 和同一客户端
> 两次串行负载验收，`global_news=true`；只输出标题、数字 ID、规范链接、发布时间和
> 证据，不读取或输出 description/正文，也不从标题推断证券身份。
> Choice/EMQuant 已完成设备激活和 API 登录，日线与日级资金流已取得
> 真实记录，Quote、盘口和分钟线仍因 `10001012/EQERR_ACCESS_INSUFFICIENCE` 待补
> 权限。iWencai 已实现正式 API Key 鉴权，当前没有授权 Key，因此真实 401 会按设计
> 非零退出，不会伪报成功。
> NBS、PBC、CFETS、FRED、IMF、World Bank、SEC EDGAR，以及新华财经、第一财经、
> 证券时报人民财讯的 Provider、严格离线解析、共享有界 HTTPS transport 和探针已实现。
> 2026-07-29，PBC 精确编目的 2024 货币供应量、CFETS Shibor/LPR/官方中间价，以及
> 新华财经/第一财经/证券时报首屏新闻通过两次 live 与串行 load，已单独开启正式能力。
> NBS、FRED、IMF、World Bank、SEC 和 CFETS DR007 仍保持 `false`；正式 trait 在
> 发起 I/O 前返回 `Unsupported`。当前 NBS landing probe 可访问但没有受支持的机器
> 序列合同，World Bank 仍受结构化 unit 缺失阻断，DR007 不用其他利率替代。

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
| `magic-market-transport` | 严格 HTTPS allowlist、无重定向/代理、整体超时、有界 body 与不持锁节流 | 不含数据源语义，不记录完整凭据 URL |
| `magic-tdx-rs` | 纯 Rust TDX 协议、同步/异步/直连/Smart 客户端、服务门面和本地文件读取器 | MoneyFlow 与集合竞价显式不支持 |
| `magic-tencent-rs` | HTTPS + GBK/JSON 的腾讯补充源，覆盖沪深京基础行情及股票/指数/ETF 行情统计 | 公共网页接口，无正式 SLA |
| `magic-sina-rs` | HTTPS + GB18030/JSON 的新浪补充源，覆盖基础行情、沪深财务三表和沪市 ETF 期权 | 历史分时、逐笔、资金流和竞价不支持；无正式 SLA |
| `magic-emquant-rs` | 通过独立 C++ bridge 调用官方 Choice/EMQuant SDK 的只读适配层 | 厂商 SDK、授权和激活文件不进入仓库 |
| `magic-eastmoney-rs` | 东财公开研报、最新财经资讯、资金流解析、龙虎榜、资本事件、涨跌停池和人气 | 与 Choice/EMQuant 身份分离；最新资讯不伪装成个股新闻 |
| `magic-cninfo-rs` | 巨潮公告/PDF 与互动易问答 | 只读公开信息；不读取账户或桌面登录态 |
| `magic-ths-rs` | 同花顺一致预期、强势原因、涨停池和热榜 | 只读公开补充源；字段/频率以当前探针为准 |
| `magic-cls-rs` | 财联社签名电报/全球新闻 | 只支持全局电报，不伪造个股过滤 |
| `magic-jin10-rs` | 金十公开 7x24 财经快讯 | 只接收未锁定的公开新闻，不请求受保护详情 |
| `magic-thepaper-rs` | 澎湃新闻财经频道原生文章 | 外链转载不归因给澎湃；不伪造个股过滤 |
| `magic-yonhap-rs` | 韩联社官方简体中文 RSS metadata | 当前 live 未准入；不抓正文、不存储、不伪造个股过滤 |
| `magic-wallstreetcn-rs` | 华尔街见闻公开 RSS metadata | 已 live 准入；只读单一 RSS，不抓正文/文章页、不存储、不伪造个股过滤 |
| `magic-baidu-rs` | 百度未复权日线和源端 MA5/10/20 | 不提供 Quote/分钟/Level-2 |
| `magic-iwencai-rs` | 获授权 API Key 的语义搜索 | 无 Key 明确鉴权失败，不复用 Cookie |
| `magic-exchange-rs` | SSE/SZSE 公告与龙虎榜、SZSE Quote/五档、HKEX 北向日统计、CFFEX 官方交割通知 | 官方公共只读端点，无 SLA；不提供 Level-2、集合竞价或 SSE Quote |
| `magic-nbs-rs` | 国家统计局经济序列的严格诊断解析 | landing 可访问但机器序列未准入；不绕过浏览器保护 |
| `magic-pbc-rs` | 人民银行已编目货币供应量 HTML | 2024 精确目录已准入；社融/地区序列不支持 |
| `magic-cfets-rs` | Shibor、LPR 与官方汇率中间价 | 三族已独立准入；DR007 不支持 |
| `magic-fred-rs` | FRED 官方经济序列 | 需要运行时 `FRED_API_KEY`；Key 永不进入证据或日志 |
| `magic-imf-rs` | IMF DataMapper 官方经济/地区序列 | 当前未准入；无时区修改时间不伪造成 UTC 来源时间 |
| `magic-worldbank-rs` | World Bank Indicators 官方分页诊断 | 结构化 unit 为空时正式能力保持关闭 |
| `magic-sec-rs` | SEC EDGAR submissions 申报元数据 | 需要描述性 `SEC_USER_AGENT`；不抓正文、附件或 XBRL |
| `magic-xinhua-rs` | 新华财经首屏新闻 metadata | 已准入；最多 13 条，不抓正文 |
| `magic-yicai-rs` | 第一财经 `firstlist` 新闻 metadata | 已准入；最多返回 50 条，不保留 notes/media |
| `magic-stcn-rs` | 证券时报人民财讯首屏快讯 metadata | 已准入；最多 30 条，不保留 content/share |
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
   ├── magic-exchange-rs ────→ magic-market-core
   ├── magic-jin10-rs ───────→ magic-market-core
   ├── magic-thepaper-rs ────→ magic-market-core
   ├── magic-yonhap-rs ──────→ magic-market-core
   ├── magic-wallstreetcn-rs ─→ magic-market-core
   ├── magic-{nbs,pbc,cfets,fred,imf,worldbank,sec}-rs
   │       ├──────────────────→ magic-market-core
   │       └──────────────────→ magic-market-transport
   ├── magic-{xinhua,yicai,stcn}-rs
   │       ├──────────────────→ magic-market-core
   │       └──────────────────→ magic-market-transport
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
| 信号与板块 | `BoardMembership`、`StrongStockReason`、龙虎榜/人气/概念记录 | Eastmoney 与 SSE/SZSE 龙虎榜、TDX 行业/概念目录和成员、Eastmoney/THS 人气、THS 强势原因实盘 |
| 资金面与筹码 | `FundFlowPoint`、`BoardFlow`、融资融券、大宗、户数、解禁、分红 | Eastmoney 除资金流 host 当前网络失败外均实盘；资金流解析/fixture 已完成 |
| 盘后资金流排行 | `PostCloseFlow`、`PostCloseFlowRequest` | Core/Router 严格合同和 Eastmoney 诊断实现完成；实网存在缺失指标与混合 `f124`，production capability 为 false、正式 trait 返回 `Unsupported` |
| 新闻/公告/互动 | `NewsItem`、`Announcement`、`InvestorQuestion` | CLS、Jin10、WallstreetCN、新华财经、第一财经、证券时报全球财经新闻，The Paper 原生财经文章，CNInfo 个股/全市场公告和互动易；Yonhap 中文 RSS metadata 已实现但 live 未准入；个股新闻仍待结构化证券身份来源 |
| 官方宏观与利率 | `EconomicObservation`、`ReferenceRateObservation`、`OfficialFxFixing` | NBS/PBC/CFETS/FRED/IMF/World Bank 严格实现与路由完成；PBC 2024 货币供应量及 CFETS Shibor/LPR/官方中间价已准入，其余按来源显式关闭 |
| 海外申报元数据 | `CompanyFiling` | SEC submissions recent/older 原子分页实现；只返回元数据与规范链接，待描述性 User-Agent 实网准入 |
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
| 证券列表/元数据 | 沪深全市场列表、finance 身份校验及实盘上市日；板块仍为派生、规则版本缺失；北京列表端点不支持 | 部分：名称/ST、派生板块；缺上市日和规则版本 | 部分：名称/ST、派生板块；缺上市日和规则版本 | 未验证，当前 capability 关闭 |
| 财务数据 | 实盘：实时 34 项、报告包和 45 个命名指标 | 不支持 | 实盘：沪深资产负债表/利润表/现金流量表，各最近 8 期 | 当前未接入统一财务契约 |
| ETF 期权 | 不支持 | 不支持 | 510050 实盘；另 3 个沪市 ETF 已实现待实测 | 不支持 |
| 除权除息 | 实盘：XDXR 全响应严格解析及标准化 `CorporateActions`；精确覆盖源定义的 1–14 类，未证明的股本/权证数量单位显式标为 `ProviderNative` | 不支持 | 不支持 | 当前未接入 |
| 板块/F10/基金 | 实盘：行业/概念/指数、F10、基金数据 | 不支持 | 不支持 | 当前未接入 |
| 全球指数/汇率 | 不支持 | 不支持 | 实盘：6 个全球指数、8 个汇率对 | 当前未接入 |
| 开盘集合竞价 | 不支持 | 不支持 | 不支持 | 不支持：完整字段集尚未证明 |

### 公共研究、内容与信号 Provider

| Provider | 已真实取得或当前诊断状态 | 当前明确边界 |
| --- | --- | --- |
| Eastmoney Web | 个股/行业研报及原始 PDF、报告级目标价及区间聚合、最新财经资讯、三类板块流、个股/全市场龙虎榜、融资融券、大宗、户数、解禁、分红、四类打板、人气；严格 15:35 资金榜仅诊断 | 目标价保留源代码+名称及 L/T 原字段；最新资讯无证券身份；关键词搜索不准入；15:35 榜实网存在缺失指标与混合 `f124`，capability 为 false、正式 trait 返回 `Unsupported` |
| CNInfo | 个股公告、完整全市场公告发现、PDF metadata、互动易问答 | 内容源，不提供行情；公告 PDF 仍只返回 URL |
| THS | 一致预期、强势原因、涨停池/原因、股票热榜 | 只声明已验证涨停池，不声明其他三类池 |
| CLS | 签名全球电报及来源时间、发布者、关联股票/主题 | 不伪造个股过滤，不是行情源 |
| Jin10 | 公开 7x24 财经快讯/文章及来源时间、主题；公开经济日历 | 排除锁定 VIP 行；不请求受保护详情；不伪造个股过滤 |
| The Paper | 财经频道原生文章、栏目、标签及来源时间 | 排除外链转载；不把文本证券名提升为结构化身份 |
| Yonhap Chinese RSS diagnostic | 7 个官方简体中文 RSS 的严格 metadata 解析已通过；2026-07-26 Rolling/Economy release 探针均在 TLS 初始化时收到 unexpected EOF | 尚未生产准入：`global_news=false`、trait 返回 `Unsupported`；只映射标题/ID/链接/时间/频道/证据，不抓正文 |
| WallstreetCN RSS | 单一公开 RSS 的严格 metadata 解析；2026-07-26 live 和两次串行 load 均通过 | `global_news=true`；只映射标题/数字 ID/链接/时间/证据，summary/content 恒缺失，不抓文章页或推断证券 |
| Xinhua Finance | 官方新闻首屏严格 metadata 解析；两次 live 和三次 load 已通过 | `global_news=true`；完整校验最多 13 行后截断，只保留标题/ID/链接/时间/栏目 |
| Yicai | 官方 `firstlist` 首屏严格 metadata 解析；两次 live 和三次 load 已通过 | `global_news=true`；不保留 NewsNotes、图片、视频或分享字段，转载保留原发布方 |
| Securities Times | 官方人民财讯 XHR 首屏严格 metadata 解析；两次 live 和三次 load 已通过 | `global_news=true`；校验双时间/双 URL/游标，不保留 content/share |
| Baidu | 华电辽能未复权日 K、MA5/10/20 | 不提供实时 Quote、分钟线或 Level-2 |
| iWencai | 正式 X-Claw 鉴权和语义结果解析 | 真实数据待合法 API Key；不读取 Cookie/桌面登录态 |
| SSE/SZSE/HKEX official | SSE/SZSE 公告与龙虎榜、SZSE Quote/五档、HKEX 沪深北向日统计及 Top10 | 不提供 SSE Quote、集合竞价、逐笔委托或 Level-2；公共端点无 SLA |
| CFFEX official diagnostic | IF/IH/IC/IM 交割通知确定性解析已通过；2026-07-27 Rustls/Native TLS 均在取得 HTTP 前失败；明文官方页只用于诊断 | 尚未生产准入：capability 为 false，生产 trait 返回 `Unsupported`；不自动降级到 HTTP |
| State Council | 国务院政策库 `gongwen`/`bumenfile` 官方文件 | 仅规范 `www.gov.cn` 文件；不是新闻或行情源 |

### 官方宏观、利率与申报元数据

| Provider | 已实现合同 | 当前生产状态 |
| --- | --- | --- |
| NBS | 有界 landing 诊断和国家/月度节点完整 coverage 解析 | `economic_series=false`；landing 当前可访问，但受支持的机器序列合同未证明 |
| PBC | 精确编目 2024 货币供应量、19×16 双语表结构、M0/M1/M2 与缺失/零值 | `economic_series=true` 仅覆盖该 2024 目录；社融和地区序列显式不支持 |
| CFETS | Shibor 八期限、LPR 1Y/5Y、闭合 25 对中间价目录与原子分页 | Shibor/LPR/FX 已分别通过 live/load；DR007 false |
| FRED | 官方 series metadata + 完整单页 observations 原子组合 | 需要 `FRED_API_KEY`，当前 false |
| IMF DataMapper | DATASET/AREA、catalog、完整 envelope/cell 校验 | 当前 false；无时区修改文字只作 revision |
| World Bank | indicator/country 全分页与稳定 source/ISO/name 身份 | 结构化 unit 阻断，当前 false |
| SEC EDGAR | recent/older submissions、冲突检测、全局分页预算与规范 Archives URL metadata | 需要描述性 `SEC_USER_AGENT`，当前 false；不抓任何文件内容 |

### 市场发现、全球与日历能力

| 能力 | Provider / 诊断入口 | 严格合同 |
| --- | --- | --- |
| 全市场龙虎榜 | Eastmoney | 完整读取交易日数据后过滤/截断；源 `TRADE_ID` 唯一；代码与名称同时保留 |
| 板块目录/成员/反查 | TDX + 名称元数据联查 | `block_fg.dat`/`block_gn.dat`；分类、成员数、重复身份和请求证券均校验；展示结果保留代码、名称及两份独立证据 |
| 全市场公告 | CNInfo | 空证券条件完整翻页；总数/页数/`hasMore` 原子一致；代码与名称同时保留 |
| 最新财经资讯 | Eastmoney | 精确滚动首屏；完整列表校验后截断；财经分类、分钟倒序、数字文章 ID |
| 全球指数 | Sina | Dow/Nasdaq/S&P 500/Nikkei/Hang Seng/FTSE，精确请求与返回数量 |
| 外汇 | Sina | USD/CNY 等 8 个已验证汇率对，保留源日期时间 |
| 经济日历 | Jin10 | 仅公开未锁 type-1；保留前值/预期/实际/修正值和重要性 |
| 官方政策 | State Council | 仅国务院官方搜索与 `www.gov.cn` 规范链接，页面上限 50 |
| 研报 PDF 正文 | Eastmoney | 精确研报身份、`application/pdf`、`%PDF-`、最大 32 MiB |
| 期货交割日历 | CFFEX 诊断实现（生产 capability 未准入） | 有界扫描官方交易通知目录及同站详情；通知必须明确 IF/IH/IC/IM、请求月份的实际交割日与交割结算价；未独立证明交割方式时保留 `NotProvided`，未证明最后交易日时保留空值；不使用公式推算；生产 trait 返回 `Unsupported` |
| 15:35 资金榜 | Eastmoney 诊断实现（生产 capability 未准入） | 中国当前日、窗口后、同一 `f124`、连续 rank、精确 limit、代码+名称；2026-07-27 主站返回空响应，延迟站同一榜内混合源时间，正式 trait 返回 `Unsupported` |
| 全市场量比/主力净流入排名 | Eastmoney 诊断实现 | 固定按源端每页 100 条完整翻页，要求沪深京、代码+名称、排序、时间与覆盖原子一致；2026-07-27 完整实网尚未通过，per-metric capability 保持 false |
| 市场宽度 | LocalAnalysis | 显式版本证券全集 + 原子 Quote 批次 + 完整涨停/跌停池；输出总数、有效/涨/跌/平、覆盖率、时间偏斜及全部输入证据，不冒充单一 Provider 原子榜 |

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

2026-07-27 当前实网已验证 finance-backed 上市日期和统一生命周期 Gateway：华电辽能
`600396`、平安银行 `000001`、贵州茅台 `600519` 的上市日与身份匹配；贵州茅台
完整 45 行 XDXR 先通过身份、日期、类别和字段校验，再投影出 2024 年两条完整
`Distribution`；1900 年范围返回带请求证据的 complete empty。协议定义的 1–14
类都有显式类别和 terms；11 类保留源端“扩缩股”的宽泛 `CapitalRescaling` 语义，
不会窄化成拆股。2–10 的四个股本数值、11/12 的 `suogu` 数值以及 13/14 的权证数量
因物理单位未被上游独立证明，保留
`UnverifiedSourceUnit::ProviderNative`，不得解释成股、手、每股比例或调整倍率。
XDXR 不提供供应商发布时间，因此企业行动仍保留 `source_at=None`，生效日不会冒充
源时间。响应另带显式中国日期 `admission_as_of`，未来请求范围或生效日会在 Core 和
Router 双层失败。

标准化 Quote 的必填当前价如果为零、负数或非有限数，会连同证券身份和源字段上下文
返回 typed error，不会用昨收或 `None` 伪装修复。关闭自动重试的未连接客户端会立即
返回显式 disconnected error，不等待连接池超时。TDX 报文的 zlib 解压只由真实
网络客户端路径执行；项目不再暴露无调用方的占位 codec API。

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
直接升级，否则可能改变编译器要求和传递依赖。依赖更新应在独立提交中重新运行完整
门禁和真实探针。

### 确定性验证

依赖已经获取后，以下命令不访问行情公网：

```bash
cargo check --workspace --all-targets --locked --offline
cargo build --workspace --all-targets --release --locked --offline
cargo test --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo doc --workspace --no-deps --locked --offline
```

完整发布门使用当前默认工具链，一次执行格式、全目标编译、全部测试、严格 Clippy、
rustdoc、doctest、链接、合规和 diff 检查：

```bash
bash tools/release/preflight.sh
```

发布前还必须运行严格覆盖率门。检查器从工作区 manifest 枚举全部生产 Rust 源文件，
按 LLVM coverage segment 计数，并排除 `#[cfg(test)]` 所属测试项；遗漏 workspace
crate、遗漏生产源、仓库外路径、重复路径或 malformed JSON 都会显式失败，内联测试
不能抬高生产覆盖率：

```bash
cargo llvm-cov clean --workspace
mkdir -p target/coverage
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo llvm-cov \
  --workspace --all-features --locked --offline \
  --json --output-path target/coverage/coverage.json \
  -- --test-threads=1
python3 tools/coverage/check_thresholds.py target/coverage/coverage.json
```

门槛是生产代码整体 `80.00%`，Core、Router、TDX 关键协议/适配/服务入口及公共
资讯 Provider 集合 `95.00%`。2026-07-29 当前版本干净 `workspace/all-features`
报告为 `45230/51259 = 88.24%` 和 `26322/27707 = 95.00%`。合同、关键集合和失败语义见
[覆盖率门说明](tools/coverage/README.md)。

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
cargo run -p magic-jin10-rs --example live_probe --release --locked --offline
cargo run -p magic-thepaper-rs --example live_probe --release --locked --offline
cargo run -p magic-yonhap-rs --example live_probe --release --locked --offline
cargo run -p magic-wallstreetcn-rs --example live_probe --release --locked --offline
cargo run -p magic-baidu-rs --example live_probe --release --locked --offline
cargo run -p magic-exchange-rs --example live_probe --release --locked --offline
cargo run -p magic-nbs-rs --example live_probe --release --locked --offline
cargo run -p magic-pbc-rs --example live_probe --release --locked --offline
cargo run -p magic-cfets-rs --example live_probe --release --locked --offline \
  -- 2026-07-20 2026-07-29
cargo run -p magic-imf-rs --example live_probe --release --locked --offline
cargo run -p magic-worldbank-rs --example live_probe --release --locked --offline \
  -- --diagnostic
cargo run -p magic-xinhua-rs --example live_probe --release --locked --offline
cargo run -p magic-yicai-rs --example live_probe --release --locked --offline
cargo run -p magic-stcn-rs --example live_probe --release --locked --offline
```

FRED 与 SEC 探针只在调用方合法配置运行时身份后运行，变量值禁止写进命令日志、
证据文件或错误文本：

```bash
FRED_API_KEY=... \
cargo run -p magic-fred-rs --example live_probe --release --locked --offline

SEC_USER_AGENT='application/version operator-contact' \
cargo run -p magic-sec-rs --example live_probe --release --locked --offline
```

每个新 Provider 都按数据族独立准入。`live_probe` 本身不是生产能力绕过；只有对应
来源满足集成文档中的两次 live、串行 load、完整证据和独立审查后才会打开能力。
当前已打开 PBC 2024 货币供应量、CFETS Shibor/LPR/官方中间价，以及
Xinhua/Yicai/STCN 全局新闻；NBS、FRED、IMF、World Bank、SEC 和 DR007 仍关闭。

CFFEX 诊断实现可以单独验收，默认示例月份为 `2026-02`；成功必须精确返回 IF/IH/IC/IM
四条由同一官方通知证明的交割事件。通知未独立说明交割方式，因此标准化
`method=NotProvided`，不会从“交割结算价”推导现金交割。BR-009 live 验收成功前，
`calendar_capabilities().futures_delivery` 保持 `false`，生产 trait 返回
`Unsupported`：

```bash
MAGIC_EXCHANGE_LIVE_OPERATION=cffex-delivery \
MAGIC_CFFEX_DELIVERY_YEAR=2026 \
MAGIC_CFFEX_DELIVERY_MONTH=2 \
MAGIC_CFFEX_TLS_BACKEND=rustls \
cargo run -p magic-exchange-rs --example live_probe --release --locked --offline
```

诊断系统 TLS 时启用 `--features native-tls` 并将
`MAGIC_CFFEX_TLS_BACKEND=native-tls`；两种 backend 都是显式选择，不会静默回退。

每个 crate 另有同名 `load_probe`。Eastmoney 最多 20 次高层数据族 attempt
（部分数据族内部包含多个 HTTP 请求），CNInfo/THS 最多 5 请求，
CLS/Jin10/The Paper/Baidu/Yonhap/WallstreetCN 最多 3 请求，official-exchange 最多 8 次 mixed 高层 attempt；这些
公共/官方网页 probe 都强制并发 1、请求间隔至少 1 秒。默认
证券和日期可以通过各 crate 文档列出的 `MAGIC_*` 环境变量覆盖。

Yonhap 默认读取 Rolling，可用 `MAGIC_YONHAP_CHANNEL` 选择 `economy` 等 7 个固定
通道，`MAGIC_YONHAP_LIMIT` 限制 1–50。`MAGIC_YONHAP_MATCH` 只对当前有界 RSS
窗口做区分大小写的本地标题匹配，不是历史搜索；当前 live 未准入时会保留真实 TLS
错误而非回退 fixture：

```bash
MAGIC_YONHAP_CHANNEL=economy MAGIC_YONHAP_MATCH='半导体' \
cargo run -p magic-yonhap-rs --example live_probe --release --locked --offline
```

WallstreetCN 只读取精确的 `https://dedicated.wallstreetcn.com/rss.xml`；
`MAGIC_WALLSTREETCN_LIMIT` 为 1–50，`MAGIC_WALLSTREETCN_MATCH` 只在当前有界
feed 上做区分大小写的本地标题匹配，`MAGIC_WALLSTREETCN_LOAD_REQUESTS` 为 1–3：

```bash
MAGIC_WALLSTREETCN_LIMIT=20 MAGIC_WALLSTREETCN_MATCH='半导体' \
cargo run -p magic-wallstreetcn-rs --example live_probe --release --locked --offline
```

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
`RouterError::attempts()` 保留完整诊断。Router 不把“5 秒”硬编码到所有数据族；
调用方只对连续竞价 Quote 显式配置
`AcceptancePolicy::with_max_source_age(Duration::from_secs(5))`。该策略使用供应商
`source_at`、纳秒精度和批次中最老记录，正好 5 秒可通过，任何更老、未来、缺失、
无时区或 record/batch 不一致都拒绝；分钟线、日线和盘后指标使用各自策略。

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
│   ├── magic-exchange-live-probe
│   ├── magic-exchange-load-probe
│   ├── magic-gov-live-probe
│   ├── magic-iwencai-live-probe
│   ├── magic-iwencai-load-probe
│   ├── magic-jin10-live-probe
│   ├── magic-jin10-load-probe
│   ├── magic-router-live-probe
│   ├── magic-sina-live-probe
│   ├── magic-sina-load-probe
│   ├── magic-tdx-live-probe
│   ├── magic-tencent-live-probe
│   ├── magic-tencent-load-probe
│   ├── magic-thepaper-live-probe
│   ├── magic-thepaper-load-probe
│   ├── magic-ths-live-probe
│   ├── magic-ths-load-probe
│   ├── magic-yonhap-live-probe
│   ├── magic-yonhap-load-probe
│   ├── magic-wallstreetcn-live-probe
│   └── magic-wallstreetcn-load-probe
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
| Eastmoney Web | macOS、Linux、Windows | `reportapi`、`push2/push2delay/push2his/push2ex`、`datacenter-web`、`emappdata` 等文档列出的 HTTPS 主机 |
| CNInfo | macOS、Linux、Windows | `www.cninfo.com.cn`、`irm.cninfo.com.cn`、`static.cninfo.com.cn` 的 HTTPS |
| THS | macOS、Linux、Windows | `basic`、`zx`、`data`、`dq.10jqka.com.cn` 的 HTTPS |
| CLS | macOS、Linux、Windows | `www.cls.cn` 的 HTTPS |
| Jin10 | macOS、Linux、Windows | `flash-api.jin10.com` 的 HTTPS |
| The Paper | macOS、Linux、Windows | `www.thepaper.cn` 的 HTTPS |
| Yonhap | macOS、Linux、Windows | `cn.yna.co.kr` 7 个固定 RSS 路径的 HTTPS；当前 live TLS 未准入 |
| WallstreetCN | macOS、Linux、Windows | `dedicated.wallstreetcn.com` 精确 `/rss.xml` 的 HTTPS |
| Baidu | macOS、Linux、Windows | `finance.pae.baidu.com` 的 HTTPS |
| SSE/SZSE/HKEX official | macOS、Linux、Windows | `query.sse.com.cn`、`www.szse.cn`、`www.hkex.com.hk` 的 HTTPS |
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
- Tencent、Sina、Eastmoney、CNInfo、THS、CLS、Jin10、The Paper、Yonhap、WallstreetCN、Baidu 和交易所公共网页端点没有本项目
  可证明的 SLA 或再分发许可，部署方必须自行确认服务条款。
- 未验证字段必须保持 `None`/`Unavailable` 或返回 `Unsupported`，不得通过猜测、
  跨源填补或模拟记录“修好”。
- Probe 输出可记录 Provider、证券代码、批次 ID、质量问题、耗时和错误码，但不得
  输出任何激活令牌或个人信息。

## 当前验收状态

以下是截至 2026-07-29 已保存的验收边界（其中既含 2026-07-27 历史结果，也含
本轮新增结果），不等同于供应商 SLA：

| 项目 | 结果 | 证据摘要 |
| --- | --- | --- |
| 默认工具链全工作区门禁 | 通过 | 2026-07-27 使用 rustc 1.95.0 / Cargo 1.95.0；check、全部测试、严格 Clippy、rustdoc/doctest、链接和合规均通过 |
| 严格生产覆盖率门 | 通过 | 2026-07-29 干净 `workspace/all-features` 报告：整体 `45230/51259 = 88.24%`；关键集合 `26322/27707 = 95.00%` |
| TDX live probe | 通过 | 沪深京基础行情、12 K 线周期、分时/逐笔、财务/XDXR、板块/基金/F10 |
| TDX lifecycle live | 通过 | 600396/000001/600519 上市日；600519 2024 两条标准化企业行动；1900 范围 verified-empty |
| Tencent live probe | 通过 | 沪深京基础行情；股票/指数/ETF 行情统计；沪深当日逐笔 |
| Tencent load probe | 通过 | mixed 100/8 为 100/100；统计 12/3 为 12/12 |
| Sina live probe | 通过 | 基础行情、三类财务报表各 8 期、510050 合约/T 型报价/Greeks/IV |
| Sina load probe | 通过 | mixed 20/4、财务 6/2、510050 期权 6/2 均零失败 |
| Router live probe | 通过 | TDX 质量拒绝被保留，Tencent 合格 Quote 被选中 |
| EMQuant live probe | 部分通过 | 登录成功；真实日线、日级资金流通过；Quote/盘口/分钟返回 `10001012`，完整 probe 按设计退出非零 |
| Eastmoney live/load | 通过（已声明能力） | live 的研报、最新财经资讯、板块、龙虎榜、资本数据、四类池和人气通过；未声明资金流/关键词新闻保持诊断 |
| CNInfo live/load | 通过 | 公告 3 条、互动易 3 条；load 3/3，最小请求起始间隔 1004 ms |
| THS live/load | 通过 | 一致预期、强势原因、涨停池、热榜；load 3/3，最小请求起始间隔 1002 ms |
| CLS live/load | 通过 | 签名电报 5 条；load 2/2、20 条记录、零失败 |
| Jin10 live | 通过 | 公开财经新闻 5 条；锁定 VIP 行被排除；短暂 21 行滚动窗口单独受界 |
| The Paper live | 通过 | 财经频道原生文章 5 条；栏目/标签、来源时间和原生 canonical URL 完整 |
| Yonhap Chinese RSS | 确定性实现通过；生产未准入 | 22 个库测试、4 个能力测试和 4 个 probe 配置测试通过；Rolling/Economy release live 均为 TLS unexpected EOF，故 capability 保持 false |
| WallstreetCN RSS live/load | 通过 | live 20 条严格 metadata；同一客户端 load 2/2、各 10 条、总耗时 7.529 秒；`global_news=true`，summary/content 恒缺失 |
| PBC 2024 money supply live/load | 通过 | 两次 live 各返回 12 个 M2 月份；Jan–Oct present、Nov–Dec missing；三次串行 load 通过；仅该精确目录 `economic_series=true` |
| CFETS Shibor/LPR/official FX live/load | 通过 | 两次 live 覆盖 Shibor ON/1W、LPR 1Y/5Y、USD/CNY 与 100JPY/CNY；三族分别完成三次串行 load；DR007 保持 false |
| Xinhua Finance live/load | 通过 | live 13/13，load 三次共 39 条，最小实际请求开始间隔 1001 ms；`global_news=true` |
| Yicai live/load | 通过 | live 50/50，load 三次共 150 条，最小实际请求开始间隔 1000 ms；`global_news=true` |
| Securities Times live/load | 通过 | live 30/30，load 三次共 90 条，最小实际请求开始间隔 1001 ms；`global_news=true` |
| NBS/IMF/World Bank diagnostics | 未准入 | NBS landing 返回 140,978 bytes 但无机器序列合同；IMF exact route 为 HTTP 403；World Bank structured unit 为空 |
| FRED/SEC identified live | 未运行 | 当前环境未配置 `FRED_API_KEY`/描述性 `SEC_USER_AGENT`；正式能力保持 false，未保存任何值 |
| Baidu live/load | 通过 | 华电辽能未复权日 K/MA；load 2/2、40 条记录、零失败 |
| SSE/SZSE/HKEX official live/load | 通过 | 2026-07-27 当前树 live 覆盖 SSE 公告/龙虎榜、SZSE 公告/Quote/五档/龙虎榜及 HKEX 两条北向日统计；load 8/8、零失败，最小请求起始间隔 1001 ms |
| Router strict 5-second quote | 通过 | 2026-07-27 13:01 连续竞价：TDX 因缺可信源时间被拒绝，Tencent 600519.SH 被选中，源龄 3613 ms |
| Eastmoney target price / THS consensus | 通过 | 600519.SH 两项均同时保留代码和“贵州茅台”名称；东财 Provider 实网返回目标价 6 样本/4 机构，`TargetPriceRouter` 的严格准入由 7 个确定性路由测试证明；THS Router 实网选中 1 条一致预期 |
| Full-market rankings | 未准入 | 源端每页上限 100；主入口传输失败时丢弃全部分页并从 `push2delay` 第 1 页重启，绝不混拼快照。5,541 行全量探针分别因部分证券缺 `f10`/`f62` 被原子拒绝；末页还有 19 个 `f124`，跨度 08:00:00–16:11:58，无法证明同一源快照，两个 per-metric capability 保持 false |
| Eastmoney strict 15:35 post-close flow | 诊断实现通过；生产未准入 | 正式 trait 返回 typed `Unsupported` 且 capability=false；当前日全量诊断因证券间源时间不一致返回 `diagnostic_probe_status=unadmitted`，不输出生产成功标记 |
| CFFEX official delivery | 诊断实现通过；生产未准入 | 确定性诊断测试精确返回 IF2602/IH2602/IC2602/IM2602；2026-07-27 双 TLS 均未取得 HTTP。官方明文页诊断确认 2026-07-17 及 IF/IH/IC/IM 结算价，但明文不进入 Provider，capability 仍为 false |
| iWencai live | 待授权 | 无 Key 的真实 HTTP 401 正确映射为脱敏鉴权错误；未伪造数据 |
| Release package | 每个提交独立构建 | 四十八个独立 probe、跟踪文档、许可证、构建元数据和 SHA-256 清单 |

任何 Provider 字段、授权、服务器或网页协议发生变化后，都必须重新运行对应的
确定性测试和真实 probe。旧验收记录不能自动证明新版本仍然可用。

## 文档索引

| 文档 | 内容 |
| --- | --- |
| [部署手册](docs/DEPLOYMENT.md) | 可重复构建、平台、网络、EMQuant runtime、健康检查、回滚与升级 |
| [TDX 能力矩阵](docs/TDX_CAPABILITIES.md) | 全数据族、北京市场、证据和显式不支持边界 |
| [TDX 生命周期实网证据](docs/evidence/2026-07-27-tdx-lifecycle.md) | 上市日期、标准化企业行动、verified-empty 与时间戳边界 |
| [排名、宽度、一致预期与目标价证据](docs/evidence/2026-07-27-rankings-consensus-target-price.md) | 代码+名称、完整分页边界、组合宽度、THS 与目标价实网 |
| [Router 5 秒新鲜度证据](docs/evidence/2026-07-27-router-freshness.md) | 连续竞价切源、最老源时间与严格纳秒边界 |
| [CFFEX 交割诊断证据](docs/evidence/2026-07-27-cffex-delivery.md) | 双 TLS、官方明文诊断与未准入边界 |
| [Tencent 接入合同](docs/integrations/tencent-web.md) | 端点、统计字段/单位、市场/周期边界与负载结果 |
| [Sina 接入合同](docs/integrations/sina-web.md) | 基础行情、财务三表、ETF 期权、字段与负载结果 |
| [Choice/EMQuant 接入](docs/integrations/eastmoney-emquant.md) | SDK bridge、激活、能力映射和当前权限状态 |
| [Eastmoney Web 接入](docs/integrations/eastmoney-web.md) | 研报、最新财经资讯、资金面、龙虎榜、打板、人气及未准入诊断 |
| [CNInfo 接入](docs/integrations/cninfo-web.md) | 证券/org 映射、公告/PDF 和互动易问答 |
| [THS 接入](docs/integrations/tonghuashun-web.md) | 一致预期、强势原因、涨停池和热榜 |
| [CLS 接入](docs/integrations/cls-web.md) | 签名全球电报、字段和限流边界 |
| [Jin10 接入](docs/integrations/jin10-web.md) | 公开财经快讯、VIP 排除、字段和限流边界 |
| [The Paper 接入](docs/integrations/thepaper-web.md) | 财经频道原生文章、转载排除和字段边界 |
| [Yonhap RSS 接入](docs/integrations/yonhap-rss.md) | 7 个官方中文 feed、metadata-only 合同、探针与当前 TLS 未准入状态 |
| [WallstreetCN RSS 接入](docs/integrations/wallstreetcn-rss.md) | 单一公开 feed、metadata-only 合同、生产准入与条款边界 |
| [NBS 官方源](docs/integrations/nbs-official.md) | 国家统计局诊断合同、历史 403/当前 landing 证据与未准入边界 |
| [PBC 官方源](docs/integrations/pbc-official.md) | 精确货币供应量目录、表结构与社融缺口 |
| [CFETS 官方源](docs/integrations/cfets-official.md) | Shibor、LPR、中间价和 DR007 边界 |
| [FRED API](docs/integrations/fred-api.md) | 运行时 Key、序列分页与缺失语义 |
| [IMF DataMapper](docs/integrations/imf-datamapper.md) | dataset/area、完整 envelope 与来源时间边界 |
| [World Bank Indicators](docs/integrations/worldbank-indicators.md) | 全分页身份与结构化 unit 阻断 |
| [SEC EDGAR](docs/integrations/sec-edgar.md) | User-Agent、公平访问、申报元数据与不抓正文 |
| [新华财经](docs/integrations/xinhua-finance.md) | 首屏 metadata-only 新闻合同 |
| [第一财经](docs/integrations/yicai-news.md) | `firstlist` metadata-only 新闻合同 |
| [证券时报](docs/integrations/securities-times.md) | 人民财讯 XHR metadata-only 合同 |
| [本轮准入证据](docs/evidence/2026-07-29-official-macro-global-news.md) | 确定性结果、准入状态与显式残余阻断 |
| [Baidu 接入](docs/integrations/baidu-web.md) | 未复权日 K 与源端 MA5/10/20 |
| [iWencai 接入](docs/integrations/iwencai-api.md) | API Key 鉴权、语义搜索和脱敏错误 |
| [交易所官方源](docs/integrations/exchange-official.md) | SSE/SZSE 公告与龙虎榜、SZSE Quote/五档、HKEX 北向日统计 |
| [授权 Level-2 集合竞价](docs/integrations/level2-auction.md) | 完整字段、Provider conformance、凭据和准入边界 |
| [券商账户边界](docs/integrations/broker-account-boundary.md) | 现金、持仓、委托、成交的独立 authenticated gateway 约束 |
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
