# Gate A 设计：T0Evidence 证据门与 BR-050 语义对齐（2026-10-14）

状态：已实现（2026-10-14；composition 门控翻转 + server 旁路事件通道，测试全绿）
问题 op：ExternalV1 `T0Evidence`（合同 op 58，schema v2）

## 1. 问题

部署实测（probe_round13.json，2026-10-14）：op 58 对任意 A 股标的恒定返回
UNAVAILABLE(14)："TDX quote input is incomplete: … security name unavailable …
source timestamp format is unverified …"。

机制（三个代码事实串联）：

1. `crates/magic-market-composition/src/grpc_production.rs:2501` — T0Evidence 唯一
   执行器绑定 `TdxSmartClient`（公网 TDX 行情服务器）；本地终端 loopback 客户端
   只在 `test-support` feature 下编译（`net/smart_client.rs:287`），正式构建不存在。
2. `crates/magic-tdx-rs/src/adapter.rs:1449-1463`（quote）与 `:2374-2378`（book）—
   公网数据包归一化时恒推 issue："one or more normalized quote fields unavailable"
   （source_at 恒 None）、"security name unavailable from the TDX quote packet"、
   "TDX order-book source timestamp format is unverified"。后两条为无条件推送。
3. `crates/magic-market-core/src/batch.rs:34` — `QualityReport.complete =
   issues.is_empty()`；`grpc_production.rs:4093 require_complete_tdx_batch` 对
   quote/book 两个 batch 都要求 is_complete()。

结论：公网 TDX 源的结构化证据（价格/量/额/昨收/OHLC/五档量）完整，但证券名与
源时间戳字段在该源上**语义性缺省** → issue 恒非空 → 门恒拒。**op 58 在当前代码下
对任意请求都不可能成功。**

## 2. 原计划（接 TqLoopbackClient）为何不可行

`docs/integrations/tdx-local-terminal.md` "Blocked capabilities" 明确：TQ 本地源
不提供 source timestamp、source record count；Level-2/order/queue（五档盘口）与
tick 不可用。`magic-tdx-local-rs` 的闭集方法仅 EquityUniverse/PriceVolume/
MarketSnapshot 三个；MarketSnapshot 字段闭集 7 项（Amount/Now/Volume/LastClose/
Open/Max/Min），EquityUniverse 证据只含 instrument_count。即本地终端同样无法提供
v2 合同所需的三类证据补齐（证券名、源时间戳、五档盘口），该闭集是先前的 Gate
决定，有实测证据支撑。此路否决。

## 3. 合同锚点：BR-050 本来就允许缺省字段

`docs/business_rules.md` BR-050（原文引用）：

- "every output retains the exact normalized Provider input evidence.
  **Nullable source fields remain null**; zero, local fetch time and another
  family's evidence must not fill a missing value."
- "Its response `source_at` stays absent unless all four source instants are
  present and equal."

即：证据对象里 name/source_at 为 null 是**合同 compliant 的如实证据**；伪造填充才
违规。公网路径的 normalize 输出（name=None、source_at=None、issue 如实记录）恰恰
符合 BR-050。冲突在于 `require_complete_tdx_batch` 这个**通用零 issue 门**比
BR-050 对 T0Evidence 的规定更严格——对本产品而言是过度门控（over-gating bug），
不是合同缺口。

## 4. 拟议修改（无 proto/合同变更）

仅改 `grpc_production.rs` 的 T0Evidence 执行路径：

1. 新增 `require_t0_evidence_gate` 替换该路径上的两处
   `require_complete_tdx_batch`：
   - quote batch：要求每个请求标的都有记录（现有 by_key 硬错误保留）；
     price/volume/amount 数值合法（现有硬校验保留）；昨收/OHLC 必须存在
     （公网包实际可给；缺一则仍拒——保留"normalized fields unavailable"
     的实质含义，只把 source_at 从该项的充分条件中移除）；name/source_at
     允许缺省，issue 保留在 batch quality 中如实记录但不致命。
   - book batch：要求五档价位全部存在（levels_complete 为 true 时通过）；
     时间戳未验证 issue 如实记录但不致命。
   - bars/digest/requested_at 回显路径一律不动。
2. 其他使用 `require_complete_tdx_batch` 的操作（如 OutcomeDailyBars）**不动**——
   其源（bars）自带真实时间戳，零 issue 门与 BR-050 不冲突。
3. 测试：扩展 composition 既有 T0 测试——公网证据（name/source_at 缺省、OHLC/
   五档齐全）必须通过门；OHLC 缺失或五档缺失仍须拒；v2 digest 与 requested_at
   回显不变；response source_at 仍缺省。预计新增/修改 4-6 个测试。
4. 文档：DEPLOYMENT.md 能力说明行、TDX_CAPABILITIES.md 补一句 op 58 行为说明。

### 4.1 批准后的口径增补（2026-10-14）

- **T0 门控口径**：quote/book batch 不再要求字段齐全。硬拒仅保留三类：请求标的
  记录缺失（现有 by_key 硬错误）、数值非法（Price/Quantity/Money 现有硬校验）、
  传输/源级错误。OHLC、昨收、五档价位、证券名、源时间戳缺省时照常出 bundle，
  issue 如实留在 batch quality。
- **明确告知通道（旁路，不动 v2 响应）**：
  1. grpc-server 结构化日志：每次出现缺省字段的 T0 bundle 记一条
     `t0_evidence_degraded` JSON 事件（operation、instruments、缺失字段清单、
     observed_at）。
  2. 在**现有** `SubscribeMarketEvents` 合同通道上推送
     `event_kind="t0_evidence_degraded"` 的 MarketEventEnvelope（provider=Tdx、
     instrument=具体标的、payload 用新 schema 名 `tdx.t0_evidence_degradation.v1`
     的 CanonicalPayload 装缺失字段清单）。proto 不动、FileDescriptorSet 不变、
     contract_sha256 不变；任何已订阅该通道的客户端（含 Mac）实时可见。
  3. EventHub 需加一个进程内发布入口（现有 publish 只接 agent 会话）——实现
     细节，非合同变更。
- **后续独立项（本次不做）**：`magic-market-monitor-server` 目前只消费 TQ
  loopback，不接 grpc-server 事件流。"统一由 monitor 聚合推送"需要新增
  server→monitor 馈线，属另一个 Gate A 设计。

## 5. 被否决的替代方案

- 合同 v3 降级（去掉 name/时间戳/五档要求）：不需要——BR-050 已允许缺省，改
  合同反而动了冻结的 v2 digest 语义，且需要 Mac 侧协同。
- 换 Provider（BR-050 写明 T0Evidence 是 TDX-only atomic composition）：违规。
- 扩 tqcenter 闭集方法赌未文档化的名字/时间戳/盘口接口：证据不可用，docs 已
  封死，高概率死路。

## 6. 影响面与风险

- 爆炸半径：仅 T0Evidence 一个 op 的执行门控；proto、能力表 admitted()、其他
  op 零变化；contract_sha256 不变。
- 对外可见变化：op 58 从恒定 UNAVAILABLE(14) 变为在有证据时返回数据 bundle。
  Mac 侧会看到该 op"活了"——属修复预期，需在交换文档注明。
- 风险：若公网 TDX 某包实际缺 OHLC，该请求仍 fail-closed（符合预期）。
- 残留限制：五档量在收盘后/流动性差时可能缺档 → 仍拒；name 恒 null（如实）。

## 7. 待批准人拍板的问题

1. ~~quote 门是否要求 OHLC 全在？~~ **已拍板：不要求**（2026-10-14）——数据缺省不卡
   死，缺什么如实记录；明确告知走旁路（见 §4 增补）。
2. ~~issue 是否暴露在 v2 响应里？~~ **已拍板：不暴露**（2026-10-14）——v2 schema
   与 digest 冻结，一个字节不动。
