# Gate A 设计：交易日历（A 股交易日）合同与 op（2026-10-14）

状态：待批准（决策点见 §7，按编号拍板后进入 Gate B）
问题 op：新增 1 个（建议 `TradingCalendar` / RPC `trading_calendar`）

## 1. 问题

本服务**没有 A 股交易日历**：无 Core 合同、无 op、无来源。合同、操作、数据源三级全空
（§2.1）。代价不是"少一个数据集"，而是**下游无法区分"停牌"与"源缺陷"**，已被迫用手工
调查代替一次查询（§3.3，有实证）。

## 2. 已核实的现状

### 2.1 三级全空

| 层 | 状态 | 出处 |
| --- | --- | --- |
| Core 合同 | `calendar.rs` 只有经济日历族 + `FuturesDeliveryCalendar`，无 `TradingCalendar` | `crates/magic-market-core/src/lib.rs:34-40` 导出清单 |
| op | 65 个 op 无一日历；`economic_calendar` / `futures_delivery` 是经济/期货日历 | `crates/magic-market-service/src/lib.rs:48-49` |
| 来源 | 唯一已知的 A 股交易日本源是 Hithink `/api/a-share/calendar/trading-days`，未入 allowlist、未接线 | `docs/integrations/unadmitted-provider-routes.md:12` |

### 2.2 现状是七处各自硬编码的近似

| # | 位置 | 近似方式 | 用途 |
| --- | --- | --- | --- |
| 1 | `crates/magic-tdx-rs/src/adapter.rs:102-111` | 仅周末 + 上午/下午窗口 | BR-023 当前时段准入门 |
| 2 | `crates/magic-tdx-rs/src/adapter.rs:293-329` | 15:00 日夜分界 | 判断"当日未成形日线" |
| 3 | `crates/magic-tdx-rs/src/net/utils.rs:50-95` | `TradingPhase{Trading,PrePost,Closed}`，注释自述"不考量交易所节假日" | 限流分档乘数 |
| 4 | `crates/magic-market-analysis/src/breadth.rs:643-683` | 09:15/09:30/11:30/13:00/15:00 切档 | `MarketSession` 判定 |
| 5 | `crates/magic-eastmoney-rs/src/market_rankings.rs:698-717` | **与 #4 逐字重复** | `MarketSession` 判定 |
| 6 | `crates/magic-market-composition/src/grpc_production.rs:3538-3543` | `09:30..=11:30` / `13:00..=15:00` | 分时形状常规时段过滤 |
| 7 | `crates/magic-tdx-rs/examples/live_probe.rs:185-200` | 与 #1 同构的探针副本 | 探针分段 |

**口径说明（避免过度结论）**：BR-023（`docs/business_rules.md:222-232`）已明文写出
"does not infer exchange holidays and does not fabricate a source date"。因此上表是
**被规则承认的已知缺口，不是违规**。它们对外只做门禁/分档，不声称是日历。

### 2.3 本设计只解决"日期维度"，不解决"时段维度"

09:30–11:30 / 13:00–15:00 是**与日期无关的交易所规则常量**，硬编码它们本身不是缺陷。
缺的只是"这一天是不是交易日"。因此 #4/#5 的逐字重复**不由本设计修复**（属独立重构项，
见 §11 T1）；本设计修复的是 #1/#2/#3/#6 在**休市日**把行情当作交易日处理的那一部分。

## 3. 生产后果分三级

- **轻**：#3 在节假日把休市当交易中，限流乘数取 1.0 而非 4.0。仅性能。
- **中**：#1/#4/#5/#6 在休市日把行情标成 `Continuous`——**等于伪造源时段标签**。
  下游拿到"连续竞价中"，但当天根本没有连续竞价。
- **重**：**下游无法区分"停牌"与"源缺陷"。** 这不是假设，是已结案的真实事故：

  `docs/evidence/2026-09-24-688277-suspension-gap.md` 记录：下游 BR-092 的 traded-day
  continuity 规则拒绝了天智航（688277）2026-07-16..07-29 的停牌缺口。结案花了整整
  一轮调查——三个源交叉验证（Tdx/Sina/Tencent 一致）+ 交易所三份公告（2026-030/031/032）。
  该文档 §"Contract reading" 的结论是：`complete=true` 只断言"声明行全部解码校验通过"，
  **不**断言"没有交易日被停牌"；traded-day 连续性检查必须由拥有它的那一方用自己的证据执行。

  **本服务没有把"哪些日是交易日"作为可查证据输出，所以下游只能自己重建。** 若有一个
  可准入的交易日历，下游只需 `交易日 ∩ [07-15, 07-30]` 一减：缺口日若全部落在非交易日，
  立刻排除源缺陷；若落在交易日，才需要停牌公告。

## 4. 关键发现：BR-051 已提供同构先例，成本低于预期

`docs/business_rules.md:834-848`（BR-051）与 `crates/magic-exchange-rs/src/cffex.rs:28-54`
已实现**"官方已公布日程 → 签入仓库的版本化修订 → 无运行时 I/O → 超出年份直接拒绝"**
的完整模式：

| 要素 | CFFEX 先例 | 出处 |
| --- | --- | --- |
| 签入的版本化数据 | `FIXED_DELIVERY_DATES: [&str; 12]` + `FIXED_SCHEDULE_REVISION` | `cffex.rs:31-54` |
| 无运行时 I/O | 注释明写 "performs no runtime HTTP" | `cffex.rs:28-30` |
| 年份门禁 | `request.year().get() != FIXED_SCHEDULE_YEAR` → `Unsupported` | `cffex.rs:456-461` |
| 禁止公式外推 | "the runtime never extends dates by a calendar formula" | `business_rules.md:839-840` |
| 准入常量 + 台账行 | `CFFEX_2026_FUTURES_DELIVERY_ADMITTED` | `cffex.rs:30`；`admissions.tsv:37` |
| 合同与 trait | `FuturesDeliveryRequest` / `FuturesDeliveryEvent` / `FuturesDeliveryCalendar` | `core/src/calendar.rs:352-450` |
| handler 与结果投影 | `FUTURES_DELIVERY_REQUEST_SCHEMA` / `..._RECORD_SCHEMA` / `..._SCHEMA_VERSION` | `grpc_production.rs:226-229, 1339-1345, 5256-5285` |

**直接推论（重要，修正早期估计）**：若走签入修订路线，则

- **不需要新的 HTTP/TLS 依赖** → 不触发 AGENTS.md 的 http-transports 门槛，
  不触发 Gate A 的传输层设计；
- **不需要新的 Provider 准入**（它不是 provider，是签入修订）；
- 官方 2026 节假日公告**已在仓库内**被引用：`holiday_calendar_url` =
  `docs/integrations/grpc-futures-delivery-v2.md:41`（国务院 2026 放假通知 PDF，亦由
  `tools/probes/futures-delivery-2026-conformance.ps1:20` 钉住）。

**代价只剩 op 本身**：proto RPC + `Operation` 变体 + bundle 重建 + 对端四字段冻结。
这一项**没有零成本退路**——65 个 op 里没有一个能承载"返回一段日期序列"，塞进
`EconomicReleaseSchedule` 就是 relabeling。这是与 TDX 板块族设计（JSON 载荷可零成本
扩展现有 op）最重要的成本差异。

## 5. 权威级别的真实边界（本设计最需要拍板的一节）

**必须先排除一个误解**：交易所并**不**发布一份"2026 年交易日清单"文件。它发布的是
① 年度休市安排公告（SSE/SZSE），② 国务院放假通知。交易日 = 工作日 − 已公告休市日，
是一个**派生结果**。

因此签入清单的权威级别是"**依官方公告、经复核的修订**"，不是"源端发布的向量"。
这与 CFFEX 不同——中金所**逐月发布**每个交割日，`FIXED_DELIVERY_DATES` 是对交易所
自身发布值的转录。**本设计的清单是推导值。**

这条边界决定了三个不可含糊的要求：

1. **修订必须可复核**：每个日期必须能追到具体公告（URL + 发布时刻），且复核方式
   写进测试，不允许"某人填了一张表"。
2. **不得由运行时公式生成**：BR-051:839-840 的禁令适用于本设计，生成过程只能发生在
   签入修订的评审环节，不能发生在 runtime。
3. **不得声称 SZSE 适用性**：`docs/integrations/research/2026-10-05-wg07-native-profile-mapping.md:59`
   已记录 Hithink 日历"does not establish Shenzhen applicability"。SSE/SZSE 实务上同历，
   但**那是假设**，必须由 D2 明确拍板是否可合并声明。

## 6. 拟议修改（推荐组合）

假定 D1=签入修订路线、D2=仅 SSE 且 `venue` 显式字段、D3=新增 op：

1. **Core 合同**（`crates/magic-market-core/src/calendar.rs`，仿 `:352-450` 形态）
   - `TradingDayRequest { year: PositiveU32, venue: TradingVenue }`，`new()` 校验
     年份区间并拒绝超出签入修订范围的年份——**与 `cffex.rs:456-461` 同构**。
   - `TradingVenue { Sse, Szse }`（取值由 D2 决定是否两个都准入）。
   - `TradingDayEntry { date: IsoDate, venue: TradingVenue, evidence: SourceEvidence }`，
     实现 `SourcedRecord`。
   - `TradingCalendar` trait（`futures_delivery_calendar` 同构）。
   - `CalendarCapabilities`（`calendar.rs:452-456`）增加 `trading_calendar: bool` 字段。
     **待验证**：该结构是否出现在冻结 schema 中（§10）。
2. **签入修订载体**：新增 `magic-exchange-rs`（或新 `magic-market-calendar`）内的
   `FIXED_TRADING_DAYS_2026: [&str; N]` + `FIXED_TRADING_CALENDAR_REVISION` +
   `TRADING_CALENDAR_2026_ADMITTED`，形态抄 `cffex.rs:31-54`。
3. **provenance**（`core/src/evidence.rs:6-11`）：`source_at` 取**公告发布时刻**，
   `observed_at` 取修订生成时刻，`batch_id` 携带修订号。语义必须写死在合同文档里——
   本族是仓库第一个**前瞻性已确定**的数据族（发布时即对全部未来日期成立），
   与其他族"`source_at` 是该值的观测时刻"不同。**这是本设计的语义风险点。**
4. **op 与投影**：新增 RPC + `Operation::TradingCalendar` + 独立
   `magic.market.trading_calendar.request` / `..._record` schema，附 `venue`、
   `calendar_revision`、各公告 URL。
5. **台账与文档**：`admissions.tsv` 增行（crate、provider 常量、`admitted`、
   `docs/integrations/exchange-official.md` 出处）；README 与 `exchange-official.md`
   补该族与"SSE 适用性"边界声明。
6. **测试**：确定性覆盖年份拒绝、venue 拒绝、修订号入 batch_id、日期序列严格递增无重复、
   每个日期可追到公告 URL；**外加一条反向测试**：证明 runtime 不会因跨年而自动外推。
7. **消费侧接线不在本设计范围**：#1/#2/#3/#6 改用日历属于后续改动，各自单独评审。

## 7. 待批准人拍板（按编号回答）

拍板结果（2026-10-15，开工指令）：**D1a / D2a / D3a / D4a**；D5 按 §10 验证结果处理——`CalendarCapabilities` 是 Core crate 层结构（`GetCapabilities` 走 proto `Capability` 消息，不经该结构），未出现在冻结 schema 中，故直接新增 `trading_calendar` 字段，proto 变更仅为新 RPC 本身；§13 的本地终端通道按建议仅用于修订复核与漂移检测，不作运行时源。以下原文保留。


- **D1 — 来源路线**：(a) 签入版本化修订（抄 BR-051，无 I/O、无新依赖、无 provider 准入，
  代价 = 新增 op）；或 (b) 接 Hithink `/api/a-share/calendar/trading-days`（新增 HTTP 依赖
  → 触碰 AGENTS.md 的 http-transports 门槛 + Gate A 传输设计 + 准入台账）。
  **推荐 (a)**：成本低一个档次，且 (b) 的端点按 F4 profile 是"参数化的滚动近一年日历"
  （`research/2026-10-05-wg07-native-profile-mapping.md:43,59`），并非年度权威向量。
- **D2 — 场所范围**：(a) 仅 `Sse` 显式声明，SZSE 留待单独准入；或 (b) 两所都准入。
  **推荐 (a)**：`research/.../2026-10-05-wg07-native-profile-mapping.md:59` 已明确
  Hithink 日历不证明深交所适用性；实务同历是假设，不应写进合同。
- **D3 — 暴露形态**：(a) 新增独立 op；或 (b) 不新增 op，只落 Core 合同 + 供内部门禁使用。
  **本项无 §4 式的零成本退路**（§4 已论证）。若 (b)，则下游仍拿不到，§3.3 的重级后果
  不解除——只有"末级"收益。**推荐 (a)**。
- **D4 — 时间粒度与形态**：(a) 按年返回该年全部交易日（`year` 请求，最大 366 条）；
  或 (b) 按日期区间返回被请求区间的交易日。**推荐 (a)**：与签入修订的"整年一个修订"边界
  天然对齐，且 (b) 会诱导调用方把它当作任意区间计算器，掩盖"只有签入年份可用"这一事实。
- **D5 — `CalendarCapabilities` 扩展**：确认可在该结构上加 `trading_calendar` 字段
  （需先完成 §10 的 schema 冻结验证）。

## 8. 被否决的替代方案

- **用星期几 + 法定节假日推算**：`docs/superpowers/specs/2026-08-01-eastmoney-provider-topn-settled-capture-design.md:53`
  已有明文禁令（"No cache, inferred holiday, calendar formula, fallback date"），
  BR-051:839-840 再次重申。否决。
- **接 Hithink 日历并 join 到其他 record**：`unadmitted-provider-routes.md:12` 与
  `hithink-fuyao.md:257-264` 否决的是**这个 join**（会伪造 provenance），而不是日历
  本身。本设计**不**做任何 join：日历只作为独立 op 输出，不由它给别的 record 标注日期。
  **需明确区分这两件事**，否则 D1 容易被误读为违反既有裁决。
- **扩展 `EconomicReleaseSchedule` 承载交易日**：relabeling，且该 op 的合同是
  "provider 发布的经济日程"，语义不同。否决。
- **复活/新增 router**：本 op 只有一个签入实现，无 failover 需求。否决。
- **在 runtime 用公式从节假日公告外推整年**：违反 BR-051。仅允许在签入修订的
  评审环节生成。否决。
- **把日历塞进现有 4 处门禁的内部常量**：不产生任何对外证据，§3.3 的重级后果不解除。否决。

## 9. 影响面与风险

- **爆炸半径**：新增 Core 合同 + 一个 trait 实现 + 一个 op + 台账行 + 文档。不改任何
  现有合同的语义，不动 `contract_sha256` 之外的既有 op。
- **对外可见变化**：**新增 op → 需要重新生成 client bundle，且对端需协同冻结新
  四字段**。这是本设计的主要协调成本，必须在 Gate A 阶段与下游确认排期。
- **语义风险（最高）**：§5 的"派生值"性质。若文档与 op 描述写成"权威交易日历"而不
  写明"依公告复核的派生修订"，下游会把一个推导结果当作源端向量使用。**必须在
  op 描述、合同文档、`admissions.tsv` 备注三处同时写明。**
- **前瞻性语义风险**：§6.3 的 `source_at` 语义是本仓首例，需在设计评审时单独确认，
  不能默认沿用其他族解释。
- **残留限制**：不含盘中临时休市/延迟开闭市（交易所可能临时公告调整）；不含
  各证券的停牌信息（停牌是证券级事实，另有来源）；不含港股通/北交所差异日
  （若与 SSE 不同历，属后续场所准入）。**本日历不能单独判定"某证券某日应否有行情"**——
  它只能判定"该日交易所是否开市"。§3.3 的完整解法是"日历 + 停牌证据"两者。
- **Gate C 提醒**：本仓无 PowerShell 7，`tools/docs/check_links.ps1` 不可运行，
  文档链接检查需人工完成。

## 10. 未验证项（不写成结论）

- 未验证 `CalendarCapabilities` 是否出现在冻结的 gRPC schema 中；若是，D5 需要走
  版本升级路径而非加字段。验证方式：检索 proto 与 client bundle 的 capability 响应定义。
- 未验证 2026 年 SSE 休市安排公告的确切 URL 与发布时刻（`research/2026-10-08-h08-...:39-40`
  给出 SSE/SZSE 公告链接，但未逐日复核）。签入修订前必须逐日核对。
- 未确认：下游 BR-092 的 traded-day continuity 规则是否愿意/能够消费新 op。若不消费，
  §3.3 的收益不兑现，D3 应倾向 (b)。
- 未验证 SSE 与 SZSE 2026 交易日是否**逐日完全一致**（D2 的事实前提）。

## 11. 价值评估与拆分建议

### 11.1 收益的两级结构

- **末级（不依赖任何外部消费方）**：本服务首次拥有"某日是否开市"的可引用证据；
  门禁与时段标注不再在休市日伪造 `Continuous`（§3 中/轻级）。即使下游什么都不做，
  这一级也成立。
- **重级（依赖下游消费）**：把 §3.3 的"停牌 vs 源缺陷"从**一轮多源+公告的手工调查**
  压缩为**一次查询 + 一个集合减法**。这一级是本设计的主要理由，但**前提是下游
  愿意改它的 BR-092 规则**。`docs/evidence/2026-09-24-688277-suspension-gap.md:88-94`
  已明确：BR-092 在本仓之外，本仓改不了它。

### 11.2 建议的批准范围

- **T1（可立即做，无需本 Gate A）**：把 #4/#5 的逐字重复切档逻辑收敛为一处，并在
  注释中写明其语义是"时段分类"而非"交易日判定"。纯重构，无合同变更。
- **T2（本设计）**：D1–D5 全部拍板后进入 Gate B。
- **T3（不做）**：任何形式的推算/缓存兜底。

### 11.3 与 TDX 板块族设计的对比

| | TDX `block_zs` 暴露 | 交易日历 |
| --- | --- | --- |
| 缺的东西 | 新数据族 | 判定基础（已被 7 处近似、已被下游手工重建） |
| 收益前提 | 需要"当前时点"消费方 | 末级收益无前提；重级收益需下游配合 |
| 复用现有 op | 可以（`CanonicalPayload` JSON，零合同成本） | **不可以**（无 op 能承载日期序列） |
| 协调成本 | 低（若 D4 不动身份格式则零） | 高（新 op → bundle 重建 + 对端冻结） |
| 来源权威性 | TDX 非指数权威源（长期要换官方） | 官方公告，签入修订，无更优源 |

**结论**：交易日历的**价值优先级高于** TDX 板块族（§3.3 是已发生的真实损失），
但**协调成本也更高**。若下游近期不会改 BR-092，则 D3 应选 (b)，先落合同不落 op，
把协调成本推迟到真有消费方时。


## 13. 补充实测（2026-10-15）：本地终端 TQ-Local 提供交易日历

当日对运行中的 TdxW.exe（TQ-Local，`http://127.0.0.1:17709/`）实测 `get_trading_dates`
（厂商文档：tqcenter/tqserver 函数，HTTP 模式可用；market 暂固定 SH、仅 A 股、依赖客户端
已下载上证指数 999999 盘后数据）：

- 请求：`{"id":N,"method":"get_trading_dates","params":{"market":"SH","start_time":"20260101","end_time":"20261231"}}`
- 2026 年至今：**186 个交易日**（20260105 → 20261014），有序、去重、全为工作日；
  休市区间与 2026 年法定安排自洽（春节 02-14→02-23、清明 04-04→04-06、劳动 05-01→05-05、
  端午 06-19→06-21、中秋 09-25→09-27、国庆 10-01→10-07，含元旦 01-02→01-04）。
- 2025 全年：243 个交易日（20250102 → 20251231）——历史回溯可用。
- `count=N` 返回截至 end_time 的最近 N 个交易日（实测 count=3 → 20261012/13/14）。
- 当日日期在客户端完成当日盘后下载前不出现在列表中（实测 20261015 尚未入列）；
  列表质量取决于客户端 999999 数据维护状态。

对 D1 的影响（供拍板参考）：

1. 该列表可直接用于**构造与复核签入修订**（D1a）：把各休市段与国务院公告逐段对照，
   把本探针调用留作可复现审计证据——运行时的零 I/O 性质与“公告 + 复核”权威口径不变。
2. 若改作**运行时实时源**，则是新的准入问题：vendor 派生口径、依赖桌面客户端存活、
   SH-only、盘后下载节奏；须走 local-terminal 族式证据与叶监视器形态，且改变 D3/D5 口径。
3. 建议维持 D1a（签入修订为运行时事实源），把本通道用作修订复核与漂移检测的辅助证据。

## 14. 实施记录（2026-10-15）

已实施（单次提交）：

1. **Core 合同**（`crates/magic-market-core/src/calendar.rs`）：`TradingVenue`、
   `TradingDayRequest`（年份 2000..=9999 校验 + 手写 Deserialize）、`TradingDayEntry`、
   `TradingCalendar` trait；`CalendarCapabilities` 增加 `trading_calendar: bool`
   （Cffex/Jin10 构造器同步补 `false`）。`TradingDayEntry` 覆写
   `evidence_source_at`/`evidence_observed_at`（本族 source_at 有值）。
2. **签入修订**（`crates/magic-exchange-rs/src/trading_calendar.rs`，新模块）：
   `FIXED_TRADING_DAYS_2026: [&str; 242]` + `FIXED_TRADING_CALENDAR_REVISION =
   "sse-trading-days-2026-v1"` + `SSE_2026_TRADING_CALENDAR_ADMITTED = true`；
   公告 URL/发布时刻常量；正式 trait 零 transport；非 2026 年份与 Szse 在 I/O 前
   typed `Unsupported`。
3. **op 与投影**：proto 增 `OPERATION_TRADING_CALENDAR = 66` + `rpc TradingCalendar`；
   `READ_OPERATIONS`、service `define_operations!`、server 宏与 `grpc_operation` 四处
   同步（顺序与 READ_OPERATIONS 尾部对齐）；composition 登记 `admitted(…, "Sse", …)`，
   schema `magic.market.trading_calendar.request` / `magic.market.trading_day` v1，
   记录投影附 venue/calendar_revision/announcement_url/holiday_calendar_url/date_basis。
4. **数据构造与复核**（§13 通道按建议用于复核）：以 SSE 公告休市段为闭集从工作日剔除，
   242 条（2026-01-05 → 2026-12-31，摘要 sha256 `d45fea80…`）；本地终端逐位交叉验证
   186/186 前缀全等（2026-10-14 边界）；复核工具
   `tools/probes/trading-calendar-crosscheck.py`。
5. **证据与文档**：`docs/evidence/2026-10-15-sse-trading-calendar-revision.md`（构造/
   复核/探针明细）；新合同文档 `grpc-trading-calendar.md`；`exchange-official.md` 表与
   验收节；`grpc-external-api.md` 计数 65→66、63→64、操作列表、schema 表与叙述；
   README；**BR-070**；`admissions.tsv` 行（2 live + 3 serial，`completed`）。
6. **测试**：exchange-rs 新增 5 项确定性测试（构造结构、年份/场所拒绝、证据语义、
   请求 wire）；composition 新增调用方模型公开回归 2 项 + 试样清单 + admitted 计数 64；
   contracts `READ_OPERATIONS.len()` 66；server 映射测试自动覆盖。

偏差记录：无（D5 按 §10 验证结论落地；均按推荐路线）。

协调提醒（部署时）：proto 新增 RPC → `contract_sha256` 变更；对端需重新生成 client
bundle 并协同冻结四字段后，本 op 才可通过正式 gRPC 路径消费。本批次尚未部署。
