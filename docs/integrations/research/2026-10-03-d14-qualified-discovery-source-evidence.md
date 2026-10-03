# D14 资格化日线发现：来源证据与下一最小接线切片

研究日期：2026-10-03，Asia/Shanghai。范围：WG07 / D14 的正式来源合同；不是新的行情抓取、准入、RPC 或生产验收。研究时读取的 VM 源码 HEAD 为 `9da925a8bc4cdd9d820af92f36a915ec35bb400f`，下文还给出关键文件的实际 bytes / SHA-256 / Git blob，避免将它们与随后开发提交混同。

## 结论

本次没有取得可构造普通 `QualifiedDailyChangeDiscovery` 的权威完整来源证据。当前最有价值的进展是确认了三条不同的来源路径和各自缺口：

- 公共 TDX TCP SecurityBar 缺证券/市场/区间/调整回显；请求外壳不能补成来源身份。
- SZSE 六份 1815 原件有小窗口的代码、日期和分页上下文，但没有连续 90 日、预期交易日全集、逐代码历史终态、修订/PIT 或生命周期全集。
- TDX 官方 TQ-Local 文档和交易所授权历史数据产品存在更强的原生身份/日线来源候选；这是可行性文档，不是本设备已获取的合格响应。

因此下一最小开发应先明确窗口语义并完成 Gate A，再实现正式消费接口上的版本化发现合同、资格校验与 typed Unavailable 原因，接向 Mac 已有 Gateway / review ledger；现有来源缺字段时不得构造 capability。不要再用孤立观察脚本、拼接两天原件或 `OutcomeDailyBars` 补成普通发现成功。具体 schema、operation 尚未选定，本报告不替代批准，也不要求为报告缺口新增一个永远 unavailable 的生产方法。主代理的[接线计划](2026-10-03-d14-qualified-discovery-next-slice.md)列出合同与来源前置。

## 证据等级与实际方法

本次实际执行了：

1. 检查 worktree 无 `.codegraph/`，再用 `rg` 查本仓原码、研究报告、准入与 HTTP 注册表；读取 AGENTS、CONTEXT、ENGINEERING_RULES、相关 business rules。所要求的 `autonomous-long-task` 在用户与本仓路径均不存在，按本次明确的只读研究授权回退。
2. 从已有 `target/authority-controls-20261002/` 完整读取、JSON 解析六份 SZSE body，并复核每份长度/哈希；再从 Mac 共享 `windows-evidence-20261002.1/authority-controls/` 读取同六份，6/6 哈希一致。两处 manifest 均为 `962055d4ce2ef6b03d7a4c5e954b1d5007761b83514eef357f53ce332ff60324`。这不是新 capture。
3. 实读 Mac 当前公开原码 `src/data_gateway/historical_bars.rs`、`grpc_source.rs`、`src/database/daily_change_review.rs` 与 2026-09-28 / 2026-09-24 D14 交接。
4. 浏览 TDX、SZSE、SSE/SSEINFO、东财量化接口的一手文档。只读取普通官方文档，不请求行情 API、17709、生产 Provider、服务/Health/业务 RPC，不读凭据。没有 Rust/合同/测试/注册表修改。
5. 保留研究失败：首两次较宽 `rg` 含不存在的目录/Windows glob，分别 exit 1；一次哈希汇总 PowerShell 的 foreach 管道语法错误 exit 1，修正后哈希读取 exit 0。官方 SZSE 技术首页/旧 PDF 的直接 open 超时或不可访问；SSE 部分旧 PDF 404/不可访问；Fuyao docs 和部分 Financial-API raw/GitHub 文档 open 不可用/404。未通过代理、TLS 或替代抓取绕过这些失败。

等级区分：

| 等级 | 本次材料 | 能证明 | 不能证明 |
| --- | --- | --- | --- |
| 实际原始响应原件 | 2026-10-02 已封存 SZSE/SSE body | 当时精确查询实际返回的字段、原生错误、分页计数 | 新日期、新版本或缺失字段语义 |
| 项目一手原码 | VM provider 与 Mac consumer | 现有实现具体如何取身份、截断、拒绝 | 上游源字段未保存前的内容或生产 wire 已执行 |
| 官方文档实际 open | TDX 帮助、东财 EMQuant R、SSEINFO 产品说明 | 文档声明的调用/字段/产品边界 | 本设备权限、真实90日响应、源 finality |
| 官方页面搜索索引 | SZSE 1.41/1.42/1.43 目录/摘要 | 查到的文档版本与公开摘要线索 | 未直接读取的完整规范、当前文件全集/交付能力 |

未依赖第三方 TDX API 包、聚合站或 Context7 来证明源合同；它们的搜索命中不列为资格依据。

## 当前 consumer 与“90 日”身份必须先收紧

Mac `HistoricalBarsGateway::pending_daily_change_confirmations_async` 当前直接返回 `daily_change_discovery_unavailable_v1`；`QualifiedDailyChangeDiscovery` 字段私有，只有 Gateway 合格输入可构造。review `identities` 当前接受 `outcome-provider-sequence-v1`，并非普通发现合同。2026-09-28 的 D14 交接明确要求独立发现输入、原始 bars、精确 identity/coverage/lifecycle 和后续同事务 ledger 接线；这里不是缺一段新闻爬虫。

精确消费入口均在 `\\Mac\Home\Desktop\Quant\stock_analysis\`：`src\data_gateway\historical_bars.rs:38` 是 capability，`:361` 是现有公开 review-only async seam，`:371` 是固定失败；`src\data_gateway\grpc_source.rs:3506` 是普通 transport seam，`:3514` 是 codes/days 请求；`src\database\daily_change_review.rs:235` / `:238` 是版本准入，`:535` 是 `discover_on_conn`。`grpc_handoffs\2026-09-28-d14-ordinary-historical-bars-discovery-vm-handoff.md:11`–`:14` 和 `:18` 是上游材料要求和接收计划。它是可公开转交的日期化需求文档，不是已经公布 method/operation/schema 的普通发现版本合同；目前所读 client-bundle 没有查到 WG07 符号或这样的正式普通发现合同。本次没有扩大搜索 Mac 私有目录，也未编辑消费原码。

更重要的是，Mac `grpc_source::daily_bars_async` 当前发送的是 `{"codes":[code],"days":days}`；2026-09-24 的真实旧请求为 `days=90`。它没有显式 start/end。不能把它静默解释为“连续 90 个自然日”或“90 个完整预期交易日”，也不能用返回第一个/最后一个 bar 反填请求边界。新发现合同必须声明窗口单位、精确 start/end、as_of、包含端点语义，并绑定请求原字节及 request_id；若需要 90 个 session，还须绑定完整的来源交易日 vector，而不是本地 `limit=90`。

上述 Mac 文件本次只读身份：

| 路径（stock_analysis 下） | SHA-256 |
| --- | --- |
| `grpc_handoffs/2026-09-28-d14-ordinary-historical-bars-discovery-vm-handoff.md` | `774ced156a5d2991d7c9db8769008882aefcf32644c210b3251b839ecde49fce` |
| `src/data_gateway/historical_bars.rs` | `389a4e58039543c85f3ed47551768217c2860ce4a6686a5d25f9f7e0d4cf8f8b` |
| `src/data_gateway/grpc_source.rs` | `5bd086a10b40fc442792913730c4293dd2b31677505b849df49ff6d44cdb345e` |
| `src/database/daily_change_review.rs` | `8f9d975adaf50d4551f33aa78ff5b0ed7f559f0e36b398d077909384e3317a5d` |
| `grpc_handoffs/2026-09-24-tdx-e2103-local-reconciliation.md` | `36288c93d71d6daf90b383307e3ea27cf4e00fdb84c2562f7ec05f91958ee135` |

这是共享原码读取身份，不是本次运行 Mac 工具或接受其测试为本次测试。

## 来源逐项核对

### 1. 公共 TDX TCP：本次没有补出原生 bar 身份

[SecurityBar 原码](../../../crates/magic-tdx-rs/src/protocol/types.rs) 包含 OHLC、vol/amount、日期时分与 datetime，没有 market/code、源 interval/adjust 或请求区间字段。[adapter](../../../crates/magic-tdx-rs/src/adapter.rs) 的 `Bar::new` 写入 `request.instrument().clone()`、`request.interval()`、`Adjustment::Unadjusted`；这些是请求/适配器标签，不是 bar packet 的原生证券回显。日期 `source_at` 也不是历史可获得时间。

BR-036 / BR-038 的 exact-page 计数和 typed cardinality rejection，证明的是请求分页纪律；短页、空页、超页在 DataBatch 前失败，不能当源历史终点/逐代码零记录终态。date range 仍被 `reject_unsupported_bar_range` 明确拒绝。不得解析 E2103、no_verified_batch 或 display text 恢复原 bar 批次。见 [现有 business rules](../../business_rules.md) 与 [D14 既有来源控制](2026-10-02-native-coverage-contract-controls.md)。

本次没有新的 TDX 原生 trace，也没有官方公共 TCP bar 协议证据证明缺失回显已改变。TDX quote 的 market/code、XDXR 的 identity 校验、SZSE 的代码回显都不能移植为 TDX SecurityBar 的身份。

### 2. SZSE 1815：原件真实，但不是90日资格

这六份原件是同一官方 report catalog 的不同请求，不是六个交易日或一个完整长窗口。[原研究](2026-10-02-native-coverage-contract-controls.md) 给出精确 URL、原始 capture UTC、HTTP status/MIME 与请求 URI hash；本次复核原件而不重新调用相应行情 URL。

| 原件 stem | Bytes / SHA-256 | 实际 body 证据 |
| --- | --- | --- |
| `szse-300005-two-days` | 3744 / `c00db4286efd1ecd6ea2eb9862a41e00e648cb489f57c3ee995e793c6931e164` | 300005，07-16/17两行；native total=2/pagecount=1/page=1 |
| `szse-300004-two-days` | 3738 / `5e04043c45e1a62716bd26483951bc68270dcad9430c9ab3d5ef4b2d4b747932` | 300004，同两日两行，不是300005身份 |
| `szse-300005-date-empty` | 3363 / `9e0bde5adaf70386aac652466b365782909ea8135fa5414246e76953231736d7` | 07-18零行、total/pagecount=0、error=null；不证明正常交易/停牌/未上市原因 |
| `szse-999999-issuer-empty` | 3363 / `c78cb023f94566d204547419750e2ef3ac64a29d1433a8edd346dfee5a116359` | condition回显999999，零行；不是证券目录认证 |
| `szse-300005-two-days-page2` | 3363 / `0618a79e255849f07e45746c6ef094ed13e7e0775c8aa0c936ef168db3f549e8` | page=2、空data，但total仍2/pagecount=1；不是query-wide empty |
| `szse-300005-over5d-page1` | 3405 / `5d5f75b43404497402bed95cd154c92fb62bc830ff145ebeb98cde21e034230b` | 07-16…30，HTTP200但native error非空、total/pagecount=0；不是完整空 |

所有年份为 2026。两份正例自己的 rows 有 `zqdm/zqjc/jyrq/qss/ks/zg/zd/ss/sdf/cjgs/cjje/syl1`；metadata.conditions 回显代码与日期。300005 的 18.59→14.87 是真实 SZSE 两日观察，**不是 TDX 90日原始批次**，也不证明上市状态或公司行动能授权 BR-171 例外。

metadata 还声明成交量/金额为所有交易方式汇总，单位为万股/万元；不得不经语义合同直接替换 TDX amount/volume。body 没有 source revision、publication instant、as_of snapshot token、全部预期 session、复权字段、停牌/上市/无事件分类或 immutable multi-window 终态。五日限制仅观察到两日成功、十五自然日拒绝，不能据此精确断言所有5日边界的历法运算。分多个短窗口只能产生带独立请求的观察；没有同一源快照与穷尽合同，不能凭合并去重宣称完整90日。

现有 [SzseClient](../../../crates/magic-exchange-rs/src/szse.rs) 的 `DRAGON_TIGER_QUERY_KEYS` 允许龙虎榜键 `txtStart/txtEnd` 等，**不含** 1815 的 `txtBeginDate/txtEndDate`；同一个 ShowReport 路径不是此新 catalog 已准入。来源 admission 和 HTTP query-policy 若以后扩大，应单独 Gate A/注册，不在本研究修改。

### 3. TDX 官方 TQ-Local：更强候选，仍缺正式资格

官方 [HTTP 文档](https://help.tdx.com.cn/quant/docs/markdown/mindoc-1hdhbmi50d038.html) 实际 open 的示例是 local-client `POST 127.0.0.1:17709/` 的 get_market_data：响应有 id、ErrorId、Value 的证券 key `688318.SH`、KlineTotal，以及 stock_page_index/count/total/has_more。这是厂商示例，不是本设备实际响应。has_more 与 stock-page 字段共处；不能当90日 bars 全历史终止或无修订保证。

尤其不能仅把 map-key 看成真正的证券身份：本仓 [既有 TQ 负控记录](../tdx-local-terminal.md) 第109–114行描述 get_pricevol 请求不存在的 `999999.SH` 却以该请求key返回上证指数数值；runtime 因此先核对完整A股目录。这个已观察负控只针对 get_pricevol，不证明 get_market_data bars 也发生同一错误，但足以否定“官方bar示例有key就已证明真实identity”的准入论证。同文第118–120行还记录一次daily-bar请求10秒无body超时；它不提供bar正文或延迟预算，也不能算今日重跑。TQ bars 不在本仓当前正式历史seam内，需独立family、版本和实际正/反原件证据。

官方 [K线文档](https://help.tdx.com.cn/quant/docs/markdown/mindoc-1ctuhthaq5qmg/mindoc-1h10g60jt68sc.html) 描述 start/end、dividend_type、fill_data；count>0 时 start_time 无效。fill_data 默认 true 表示填补缺数据，因此若日后做原始发现，需明确禁填、原样保留 factor/请求设置、核实同版本响应与每代码计数；不能从 count=90 得到显式90日范围。

[交易日列表文档](https://help.tdx.com.cn/quant/docs/markdown/mindoc-1ctuhthaq5qmg/mindoc-1h10q7i3702rk.html) 暂固定 SH，需预下载上证指数盘后数据。这不是 SZSE 源历史状态全集。[证券基本信息](https://help.tdx.com.cn/quant/docs/markdown/mindoc-1ctuhthaq5qmg/mindoc-1h10jj7r7jol4.html) 有 J_start / IsSTGP / IsQuitGP / TodayDRFlag，但未给这组字段逐日有效边界、历史修订和空状态覆盖。[分红送配文档](https://help.tdx.com.cn/quant/docs/markdown/mindoc-1ctuhthaq5qmg/mindoc-1h10hsiat36k4.html) 说明三类事件和参数，未给全部生命周期、更正撤回或 certified no-event。

[版本更新说明](https://help.tdx.com.cn/quant/docs/markdown/mindoc-1cfsjkbf8f3is/TdxQuantVersion.html) 当前声明 2026-09-30 tqcenter/tdxaidata 1.2.2；还列有 K 线 volume/factor 修复。这说明以后实际 capture 要绑定设备终端/SDK版本；不能把最新网页覆盖旧原件。[本仓本地终端文档](../tdx-local-terminal.md) 与 [admissions](../admissions.tsv) 目前不是该 bar family 的 qualification。本次没有启动终端/poller，没有17709请求，没有新 provider admission。

### 4. 现有 Hithink / EMQuant：身份相对更强，剩余字段不能推导

[Hithink coverage 原码](../../../crates/magic-hithink-rs/src/historical_coverage.rs) 保留实际 native thscode/interval/adjust/request_id/timestamp_ms 与 body receipt，显式 source_exhaustion/calendar/missing-reason Unknown、revision/publication NotProvided、pit=false。[现有版本2合同](../grpc-historical-bars-coverage-v2.md) 是 observation-only，不能因 HistoricalBars admission=true 就自动资格化 D14。官方 Financial-API 主页和部分官方文档搜索索引可查，但本轮多份 docs open 404/不可用；因此不声称新官方合同已重新确认终态。

[EMQuant API 官方 R文档](https://quantapi.eastmoney.com/Upload/EMQuantAPI_R.html) 描述 csd 的返回 code/date 与 Period/AdjustFlag/Market/filldata 参数；本仓 [bridge](../../../tools/emquant/snapshot_bridge.cpp) 从 SDK `data->codeArray` 写入 code，[adapter](../../../crates/magic-emquant-rs/src/lib.rs) 验证返回 code，再做本地 limit。它不是 TDX 请求身份，但本次未取得 CSD90日实际响应，也未见来源 publication/revision/as_of 及逐 session missing reason；不能把代码数组、参数或数据日期当这些证据，也不能借用其他数据 family 的发布时间。东财“掘金” history_bars 文档不能混作本仓 Choice EMQuant csd 合同。

### 5. 交易所授权历史文件：应获取合同与原件，不是公网爬虫替代

[SSEINFO 历史数据产品页](https://www.sseinfo.com/services/assortment/historical/) 说明日K/分钟K与证券基本信息；实际 open 的 [智能数据产品说明书2.0.0](https://www.sseinfo.com/services/assortment/znsj/znfwnr/c/10790553/files/01e3eccd5e4f41a3a0f960b859fb47e8.pdf) 标注2025-01-03完成、每日增量盘后18:00用Rsync，并指向《行情历史数据文件结构说明》和《Rsync数据同步服务用户手册》。本次未获得这两份结构/交付手册或实际90日文件、清单、修订/撤回流；18:00是产品交付说明，不是每条bar历史publication/finality证明。沪市产品也不能补300005深市缺口。

SZSE [技术服务目录](https://www.szse.cn/marketServices/technicalservice/) 的本次搜索索引列Ver1.43（2026-09-18）；[对应官方技术通知](https://www.szse.cn/marketServices/technicalservice/notice/t20260918_622917.html) 引用该版本。旧 [Ver1.41官方PDF索引](https://www.szse.cn/www/marketServices/technicalservice/interface/P020250328367238567039.pdf) 摘要有FTS、静态参考和两遍盘前文件，但直接open超时，未完整读取。不能把旧schema/文件名直接当2026现行90日归档服务；更不能猜FTS公共下载端点。

[SSEINFO 授权声明](https://www.sseinfo.com/aboutus/authstatement/) 是来源方关于接收/使用/经营许可的说明；公开可浏览不等于已有机器采集/再分发授权。应向实际数据供应方索取适用版本、许可范围和原始材料。此处是接入前置，不是本次授权采购、部署或法律结论。

## 下一最小可接线方案（提案，非实施回执）

第一步不需要新增 Provider 网络能力：

1. 在既有服务/contract 边界定义独立的普通发现版本，不改变 HistoricalBars v1，也不将 Hithink observation v2 或 outcome-provider-sequence-v1 当新资格。明确源选择、instrument、窗口单位/精确边界、as_of、request_id、原请求摘要和预期 session 证据引用。
2. 用一个 core-owned typed 资格验证器保持“实际源证据”和“请求/provenance”分开。返回封闭的缺口分类，不接受 caller `complete=true`、本地日期数组或空data替代 source terminal。建议缺口至少区分 SourceIdentityNotProvided、RequestWindowNotSourceBound、ExpectedSessionsNotProvided、PerInstrumentTerminalNotProvided、PublicationRevisionNotProvided、AdjustmentNotSourceBound、LifecycleCoverageNotProvided；这些是设计建议名称，不声称当前已存在。
3. 现有 provider 能交付多少实际证据就提交多少内部观察，但正式 service handler 对缺口返回 typed Unavailable，并带精确 provider/reason；不把观察success包当 qualified。这条失败路径应在注册服务、版本解析、序列化和 Mac 的现有 gateway 接线验收，不仅测孤立 helper。
4. Mac 后续 adapter 只从完整、同版本、独立校验的输入创建 QualifiedDailyChangeDiscovery，再调用现有 discover_on_conn / CLI review/token。只读发现不写人工确认、不返回 AdmittedDailyBars。未知版本、错request/证券、页空、缺session、伪终态、过期/未来as_of、错adjust、缺生命周期/原件皆拒绝。没有来源原件时只做合成fixtures的正/反合同测试，标明合成，绝不宣称300005真实90日已验收。

资格材料应逐项能核对：

| 项目 | 必需的真实来源材料 | 当前缺口 |
| --- | --- | --- |
| 原生身份与请求 | provider返回证券/市场或可验证证券目录、interval/adjust、请求原字节及原始body、源版本 | TDX bar无identity；SZSE仅小窗口；新候选仅docs |
| 精确窗口与终态 | 90单位/边界、完整页/分窗集合、逐代码total/terminal、同revision快照 | 旧days=90无边界；页空/五日拒绝不能提供 |
| expected sessions | 来源market+适用期日历vector、缺bar每日期明确状态/原因与证据 | 不能用weekday/当前本地calendar/SH指数日期替代 |
| publication/revision/as_of | 历史公开可得时间及精度/时区、版本关联、更正撤回和选择as_of规则 | row date/HTTP receipt/18:00产品说明不提供 |
| 复权与生命周期 | 原生adjust/基点/因子版本；上市有效期；all-action和无事件/终态证明 | Source参数、当前ST/上市date或三个事件类型不等全集 |

第二步才是来源接入选择：优先请求已授权供应方的明确合同和原件（若选300005，必须有深市覆盖）。可考虑TQ-Local真实只读条目或既有Hithink/EMQuant扩展，但新family/新query键、source qualification、capture和实际运行需各自Gate A/准入/执行权限。本任务没有提供这些材料，不默认“有API key就全部有权/有字段”。纯文档说明或预期完善的provider合同不应让第一步的生产失败门打开。

## 可复核原码身份

下表是本次实际读文件身份，不是新部署或二进制身份：

| 文件 | Bytes | SHA-256 | Git blob |
| --- | ---: | --- | --- |
| `crates/magic-tdx-rs/src/protocol/types.rs` | 8763 | `18b82e9105bb19b733c5447ae0c577476c0d8c0f8ff1f776108f54e7ba48f95e` | `da5aa2b4749991f7fdb019a343c67693852a3b69` |
| `crates/magic-tdx-rs/src/adapter.rs` | 91014 | `5af36dea04053a6a45581188c62215f6db56e126f1acac33c37463b76518371e` | `a8f3bddb4905d93f1569bb64aeda9ccb50372668` |
| `crates/magic-hithink-rs/src/historical_coverage.rs` | 4590 | `8f7365a19919078daa8d0026fec28789e5f9d73ed4ac6991ec08a58b1f08c684` | `391c6b26fb15f5bc6857ab95fdbb14d672861310` |
| `crates/magic-exchange-rs/src/szse.rs` | 23436 | `534d79b8b7aa3b1c8f0398a3d6bf09599f46e330f1ccdfce6f48a4db30155c87` | `2a57f0226e29ef87baedbbd0a03cad20456e2684` |
| `crates/magic-emquant-rs/src/lib.rs` | 49530 | `f37ace45ac9869deb044f5220ebecc4af61715d6784deae7117a405ea852933a` | `ae2afd4978c63dce087571a8f466191936c7659d` |
| `tools/emquant/snapshot_bridge.cpp` | 12029 | `b83205baa1ce1b02f5789e0d0cac7a0145c4261d99d3d1da0bef0bb4751178eb` | `a17a456b9d9b0fa8af1721a33232391da96e280e` |
| `docs/integrations/admissions.tsv` | 8954 | `5f5ca85cc869cf7f123e74b01c8fcacb4d9fd52d51d3426879d6001dca65a699` | `dfc81fd2e3bc182523f57f90ddd5b24164992f00` |

## 独立 remaining

D17 的第一方逐日全状态 universe / 有效边界 / 空结果含义、D20 的全公司行动生命周期与更正/无事件、R08 的来源确认和PIT，继续各自开放。D14局部研究不为它们授资格，也不关闭完整科技/财务新闻覆盖。b7/f23旧binary、来源admission、listener/36RPC/凭据/停止/生产发布边界不变。本报告只新增这一份研究文档，未执行代码测试/构建或真实RPC，不能计为平台完成。
