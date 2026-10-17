# Gate A 设计：QualifiedTradingFacts（provider-sourced 观察值）（2026-10-17）

状态：待批准（决策点见 §5，按编号拍板后进入 Gate B）

问题 op：拟新增 1 个**组合级** op（建议 Operation **67**；编号随拍板，不预占）。不改任何既有 op 的请求/记录 schema；唯一 proto 变更为新增 RPC 与其枚举值。实现形态复用已准入的 TDX `SecurityMetadata` 与腾讯行情批次，**不新增 provider、不新增 HTTP 依赖**。

---

## 1. 问题与依据

- 来单链：`grpc_handoffs/2026-10-17-windows-data-gaps-and-intraday-alert-request.md`（P0-1）→ 我方回执 `2026-10-17-p01-provider-sourced-trading-facts-evaluation.md`（2/3 子集可交付）→ Mac 确认 `2026-10-17-p01-spec-confirmation.md`（两点全接受，要求起草 Gate A 并明确三件事：`price_band_source` 枚举值、`Unsupported` 的 typed 形状、`listing_state` 派生规则）。
- 交付范围（已确认）：`lifecycle`（listed_on / listing_state）+ `price_band`（**仅观测快照日**）；`suspension` **明确不交付**（本侧零停牌来源，10-15 回执与全仓核查一致），显式请求该子集返回 typed `Unsupported`。
- 目的：解封 Mac 侧"候选入池 recorded=0 / 快照 I-03/T-03"中依赖生命周期与价格带的判断；消费侧按 `provider_observed` 降级使用。

## 2. 现状核实（2026-10-17：实测 / 静态）

### 2.1 TDX SecurityMetadata（lifecycle 来源）——实测

`SecurityMetadata {provider=Tdx}`（600519）实测帧：`listed_on="2001-08-27"`、`price_limit={"percent":null,"version":null}`、`status="Unavailable"`、`source_at=null`、`observed_at="1792203685"`（unix 秒字符串）、`batch_id="tdx:1792203685:security-metadata"`。`admission=1`、`grpc_status=0`。
静态：记录结构 `crates/magic-market-core/src/provider.rs:978-991`（listed_on / price_limit / status / source_at / observed_at / batch_id）；`listed_on` 来自 finance `ipo_date`（`crates/magic-tdx-rs/src/adapter.rs:1844-1909` 的 `normalize_ipo_date`）；注册于 `grpc_production.rs:2841-2866`（`admitted(Operation::SecurityMetadata, "Tdx", …)`）。

### 2.2 腾讯 MarketStatistics（price_band 来源）——实测 + 静态

`MarketStatistics {provider=Tencent}`（600519）实测记录**已直接携带** `upper_limit=1444.27`、`lower_limit=1181.67`、`turnover_rate`、`trailing_pe`、`static_pe`、`pb`、总/流通市值、`volume_ratio` 与完整 `evidence{provider=Tencent, source_at=2026-10-16T16:14:47+08:00, observed_at=1792203916.722327600, batch_id=tencent-web:1792203916.722327600:market-statistics}`。涨跌停由腾讯公开行情字段解析并做上下限一致性校验（`crates/magic-tencent-rs/src/lib.rs:213-214、535-536`），随该记录上 wire。
`RealtimeQuotes` 记录实测不含涨跌停字段（字段清单见探针帧），本 op 不使用它。
请求上限 `1..=50 unique`（`grpc_production.rs:278-279`）。

### 2.3 请求/记录基础类型——静态

`InstrumentsRequest { instruments: Vec<InstrumentId> }`（`grpc_production.rs:347-349`）；`InstrumentId` 为现有冻结合同类型。交易日历：`TradingCalendar`（op 66，SSE 签入修订）已在线上，可用于 dates 校验。

### 2.4 typed 错误形状——静态（回答来单 Q2）

- 结构化错误详情 `ErrorDetail`：`crates/magic-market-grpc-contracts/proto/magic/market/v1/market.proto:173-185`（request_id / operation / provider / reason_code / retryable / admission / evidence_code / evidence_field / record_index / has_record_index / provider_attempts），经 gRPC **binary metadata** 键 `magic-error-detail-bin` 下发（`crates/magic-market-grpc-server/src/app.rs:20`、`:1101-1149`）。
- `ServiceError::Unsupported` → `Code::Unimplemented` + `reason_code="capability_unadmitted"` + `retryable=false`（`app.rs:846-857`）；`admission` 字段为 `Unadmitted`；message 携带原因原文（如 `no admitted suspension source`）。
- 消费侧可据此按 `reason_code` 精确分支，**无需字符串匹配**。

### 2.5 边界事实——静态

- 无停牌来源（全 provider crate 核查；竞价快照 `data_status=suspended` 只在竞价窗口且无 false 语义）；无历史价格带来源；无退市日期来源（Hithink 的 `delisting_date` 属未知字段，解析即拒：`crates/magic-hithink-rs/src/metadata.rs:446-450`）。
- op 新增先例 `ce7cb59`（TradingCalendar=66）：23 个文件面 + `contract_sha256` 变化 → Mac 侧 fail-closed 直至 bundle 重生成并重钉（该提交的协调注记原文仍适用）。

## 3. 拟议变更（拍板后实施）

### 3.1 合同面

- proto：新增 RPC（建议 `QualifiedTradingFacts`）+ 枚举值（下一个可用号 **67**）；
- `READ_OPERATIONS` +1（长度断言 66→67，`crates/magic-market-grpc-contracts/src/lib.rs:15、:271`）；
- `Operation::QualifiedTradingFacts` +1（`define_operations!`，`crates/magic-market-service/src/lib.rs:18` 起）；`app.rs` `grpc_operation` 映射 +1（`:684` 起）；
- 请求 schema（建议名）：`magic.market.qualified_trading_facts.request` @ v1；
- 记录 schema（建议单数名，回答来单"用单数"要求）：`magic.market.qualified_trading_fact` @ v1；
- 文档计数：`grpc-external-api.md` 66→67 RPC、64→65 handler-backed（现文第 12 行），op 表与 schema 表同步；新增合同文档 `docs/integrations/grpc-qualified-trading-facts.md`。

### 3.2 请求

```json
{ "instruments": [ {"exchange":"Shanghai","code":"600519","asset_class":"Equity"} ],
  "dates": ["2026-10-16"],
  "include": ["lifecycle","price_band"] }
```

边界（逐项校验，越界在 provider 调用前 typed `InvalidRequest`）：

- `instruments`：1..=50，唯一，仅 A 股个股权益（`AssetClass::Equity`）；
- `dates`：1..=250，唯一、升序无关（内部排序），且必须 ⊆ **SSE 2026 签入交易日历**（超出 → `InvalidRequest`）；
- 行数：`instruments × dates ≤ 500`（部署 payload 上限 512 KiB 预算，见 §4）；
- `include`：子集 ⊆ {`lifecycle`,`price_band`}，缺省 = 两者；**显式包含 `suspension` → typed `Unsupported`**（reason: `no admitted suspension source`）。

### 3.3 记录（每个 (instrument, date) 一条；排序 = instruments 输入序 × dates 升序）

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `instrument` / `date` | object / string | 同请求；date 为 `YYYY-MM-DD` |
| `quality` | string | v1 恒为 `provider_observed`（不产出 `authoritative`） |
| `source` | string | lifecycle 来源，v1 恒为 `tdx-security-metadata` |
| `observed_at` | string | RFC3339 带时区（+08:00），由内部批次 observed_at 转换（§3.4-D8） |
| `source_at` | string \| null | 恒为 null（TDX 元数据无源时间，BR-050 不代填） |
| `listed_on` | string \| null | 源值原样 `YYYY-MM-DD` |
| `listing_state` | string | `listed` / `not_yet_listed` / `unknown`（**v1 不产出 `delisted`**） |
| `limit_up` / `limit_down` | number \| null | 腾讯批次源值；仅快照日有值 |
| `price_band_source` | string \| null | 有值 = `tencent-market-statistics`（该行价格带可用）；null = 该日不可用 |
| `evidence` | object | lifecycle 的 `SourceEvidence`（provider=`Tdx`、batch_id、observed_at 原字符串） |
| `price_band_evidence` | object \| null | **可选新增字段**（D4）：价格带批次的 `SourceEvidence`（provider=`Tencent`、source_at、batch_id；透传统计记录自身 evidence） |

**价格带"快照日"的精确定义**：`date == 腾讯批次 source_at 的 Asia/Shanghai 自然日`（非"服务器当天"；节假日查询时快照日自动落到最近交易日）。请求的 `dates` 不含该日 → 全部 `limit_*`/`price_band_source`/`price_band_evidence` 为 null，批仍为完整。

**`listing_state` 派生规则（回答来单 Q3）**：

- `listed_on` 非空且 `≤ date` → `listed`；
- `listed_on` 非空且 `> date` → `not_yet_listed`；
- `listed_on` 为空 → `unknown`；
- `delisted` 永不产出（无退市来源）；「`listed`」仅表示按上市日的上市状态成立，**不构成"在交易"证明**（无停牌源），消费侧不得把 `listed` 当 trading。

### 3.4 对来单三问的答复（拍板确认项，D7）

1. **`price_band_source` 枚举值**（v1 封闭集）：`"tencent-market-statistics"` | null；`source` 枚举（v1 封闭集）：`"tdx-security-metadata"`；`quality` 枚举（v1）：`"provider_observed"`。
2. **`Unsupported` 的 typed 形状**：gRPC `UNIMPLEMENTED` + binary metadata `magic-error-detail-bin` 解码为 `ErrorDetail{ reason_code:"capability_unadmitted", retryable:false, admission:Unadmitted, operation:QUALIFIED_TRADING_FACTS, request_id }`，message 携带原因原文（§2.4）。
3. **`listing_state` 派生规则**：见 §3.3（`listed` / `not_yet_listed` / `unknown`；`delisted` 不产出；`listed ≠ trading`）。

### 3.5 组合实现与失败语义

- 处理器（composition 层，无 provider 行）：解码请求 → **一次** `SecurityMetadata{provider=Tdx}`（全部 instruments，≤50）→ 若 `include` 含 `price_band` 且 `dates` 含快照日：**一次** `MarketStatistics{provider=Tencent}` → 组装行（快照日由该批次 `source_at` 的上海自然日决定）。
- **失败语义（D5）**：任一内部批次 typed 错误或非 complete → 整个 op **typed 失败**（fail-closed）；不产出"部分行 + 静默 null"。内部批次证据进错误详情（`provider_attempts`）。
- 确定性：行序固定（§3.3）；同一请求两次调用除 `observed_at`/`batch_id` 外应逐字段一致（价格带随行情变动属预期）。

### 3.6 规则

新增 **BR-071**（现最高 BR-070）：把 op 边界写成规则——`quality` 单值且 v1 仅 `provider_observed`；`suspension` 不交付（显式请求 → typed `Unsupported`）；价格带仅源值、禁止推导、仅快照日；`listing_state` 派生规则与 `listed ≠ trading`；行数/日历边界；`complete` 语义（空 instruments → 拒绝；标的缺失 → typed 失败，不静默省略行）。

### 3.7 明确不做

- 不交付 `suspension`（任何形态）；不交付历史日期的价格带；不产出 `delisted`/`authoritative`；不做价格带推导；不存储/缓存；不改动 `SecurityMetadata` / `RealtimeQuotes` 的既有记录 schema。

## 4. 影响面与风险

| 变更 | 涉及文件 | 风险 |
| --- | --- | --- |
| 新 op（proto 枚举+RPC） | `market.proto`、`READ_OPERATIONS`（+长度断言）、`Operation`、`app.rs` 映射 | 中：**`contract_sha256` 变化 → Mac 对旧 bundle fail-closed，需重生成 client-bundle 并重钉**（ce7cb59 同款协调注记） |
| composition 处理器 + 记录投影 | `grpc_production.rs`（注册+处理器+schema 常量）、覆盖测试 | 低-中：两次内部批次串行（metadata ~1-2s + quote ~1s）；500 行 payload 需实测（估算 150–250 KB < 512 KiB） |
| 规则/文档 | `business_rules.md`（BR-071）、`grpc-external-api.md`、新合同文档、README | 低 |
| 证据 | Gate C：live 探针 ×2 + serial ×3（含 negative：`include=suspension`→Unimplemented；日历外日期→InvalidArgument；缺失标的→typed） | 低；无 admissions.tsv provider 行（组合级 op） |

## 5. 决策点（按编号拍板）

**D1 — 出口形态。** (a) 新增组合级 op【推荐：契约面最小、复用已准入来源】；(b) 扩展 `SecurityMetadata` 记录加字段（改冻结合同且波及全部现有调用方，否决）；(c) 不交付（来单已确认要交付）。

**D2 — 命名。** (a) op=`QualifiedTradingFacts`、请求 `magic.market.qualified_trading_facts.request`、记录 `magic.market.qualified_trading_fact`【推荐，与来单一致】；(b) op=`TradingFactsObservation`（备选）。

**D3 — 请求界。** (a) 与来单一致：instruments≤50、dates≤250 且行数≤500、dates ⊆ SSE 2026 日历、`include` 缺省两子集、显式 `suspension`→`Unsupported`【推荐】；(b) 更小 dates 上限（如 ≤50）。

**D4 — 证据形态。** (a) 单记录 + 可选 `price_band_evidence`【推荐：两来源批次证据都保留】；(b) 按子集拆两条记录；(c) 只带 `price_band_source` 不带批次（否决：丢可复现证据）。

**D5 — 失败语义。** (a) 任一内部批次失败 → 整 op typed 失败【推荐，fail-closed】；(b) 降级为 null（否决：静默降级）。

**D6 — 规则。** (a) 新增 BR-071【推荐】；(b) 仅文档、不立规则。

**D7 — 三问口径确认（来单 Q1/Q2/Q3 的最终值）。** (a) 按 §3.4 三值枚举 + ErrorDetail 形状 + 派生规则【推荐】；(b) 有修改（请注明）。

**D8 — `observed_at` 格式。** (a) RFC3339 `+08:00`，毫秒精度，由内部批次 unix 值转换；`evidence` 内保留原字符串【推荐】；(b) UTC `Z`；(c) 保留 unix 字符串（与来单 ISO 要求冲突，否决）。

## 6. 未验证项（Gate C 前不写成结论）

1. ~~腾讯 `upper_limit`/`lower_limit` 的投影来源~~ **已解决（2026-10-17 实测）**：`MarketStatistics` 记录已携带涨跌停与完整证据，新 op 直接投影，不再读取 `RealtimeQuotes` 客户端内部字段。剩余：含停牌票/无价带票时的源行为。
2. TDX `SecurityMetadata` 的 instruments 实际受理上限（scope 未写数字；Gate B 需实测 50 票批次）。
3. **停牌票在 TDX 元数据中的行为**（缺失 → typed 失败 vs 字段 null）——直接决定"请求含停牌票时 lifecycle 能否交付"，需在有停牌样本时验证。
4. 500 行 payload 实测字节数（401 票 × 日…按 §3.2 = 最多 500 行）。
5. 非交易日查询的 `price_band` 语义（快照日=最近交易日）需实测确认（周末/节假日各一次）。

## 7. 价值评估与拆分

- 价值：解封 Mac 候选入池的 lifecycle/价带依赖（provider_observed 降级语义）；零新源、零存储、零既有合同变更；同时把"`listed ≠ trading`"的边界写死在记录语义里。
- 不做它的代价：Mac 侧继续 recorded=0（候选入池整条链路不可用）。
- 拆分（按 Gate 顺序）：
  1. 合同批次（proto/READ_OPERATIONS/Operation/app.rs + 长度测试）；
  2. composition 批次（处理器 + 记录投影 + 请求校验 + 覆盖测试）；
  3. 规则文档批次（BR-071、grpc-external-api.md、新合同文档、README）；
  4. 证据批次（Gate C：live×2 + serial×3 + negative 探针 + 证据文档）；
  5. 部署批次（Gate D：rebuild + cutover + bundle 重生成 + Mac 重钉 + 消费侧断言更新）。

## 8. 拍板与实施记录

**拍板结果（2026-10-17，全案）：D1a / D2a / D3a / D4a / D5a / D6a / D7a / D8a。**
- op 名与 schema 按 D2a；编号 67 在合同批次中分配（本设计此前不预占，拍板后落号）。
- 之后按批次实施：合同 → composition → 规则文档 → 证据（Gate C）→ 部署（Gate D，含 bundle 重生成与 Mac 重钉）。

## 8. 拍板与实施记录

**拍板结果（2026-10-17，全案）：D1a / D2a / D3a / D4a / D5a / D6a / D7a / D8a。**
- op 名与 schema 按 D2a；编号 67 在合同批次中分配（本设计此前不预占，拍板后落号）。
- 之后按批次实施：合同 → composition → 规则文档 → 证据（Gate C）→ 部署（Gate D，含 bundle 重生成与 Mac 重钉）。

**实施记录（2026-10-17，未部署）**
- 合同面：`proto`（`OPERATION_QUALIFIED_TRADING_FACTS`=67 + RPC）、`READ_OPERATIONS`（67）、`Operation::QualifiedTradingFacts`、server RPC 与 `grpc_operation` 映射。
- composition：新增 `register_qualified_trading_facts` + `execute_qualified_trading_facts`（内部 TDX `SecurityMetadata` + 腾讯 `MarketStatistics`）；请求校验（50/250/500/日历子集/include）、`listing_state` 派生、快照日价格带、`observed_at` 的 RFC3339(+08:00) 转换、fail-closed 缺标处理。
- 实测校正（D7 范围内的字符串）：`price_band_source` 取值由设计初稿的 `tencent-quote-batch` 校正为 **`tencent-market-statistics`**（依据 §2.2 实测：该记录是涨跌停的既有 wire 来源）。
- 测试：`magic-market-composition` 新增 13 个单测（校验矩阵 + 投影 + 时间转换）全绿；覆盖率矩阵补入 op 67。
- 文档：新增 `grpc-qualified-trading-facts.md`；`grpc-external-api.md` 计数 67/65 与 op 表、§16；BR-071；bundle 脚本 RPC 断言 66→67。
- 待办：Gate C 活体证据（部署实例上的正/负探针）、Gate D 部署 + bundle 重生成 + Mac 重钉。
