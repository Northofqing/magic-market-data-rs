# Gate A 设计：准入索引覆盖率与阻塞分类（2026-10-14）

状态：待批准（决策点见 §7，按编号拍板后进入 Gate B）
问题 op：无。本设计**不新增、不修改任何 op 的运行时行为**（除非 D2 选 (a)，见 §6.2）

## 1. 问题

第八个问题（授权阻塞源）的原始表述是"EmQuant、CFETS DR007、IMF、GACC 四个被授权阻塞的源"。
核查后这个表述在**两个方向上都偏离事实**：

- 真正 `admitted=false` 的是 **9 行 / 7 个 crate**，其中**只有 1 个**（CFETS DR007）是真正的
  第三方授权问题；
- EmQuant 在台账里是 `admitted=true`，但仓内最新的实网证据显示它的 API 授权已过期。

顺着这两条偏差查下去，暴露出的是**准入证据体系的结构问题**：它有三个互不交叉的事实来源，
而 README 的权威声明只指向其中两个。

## 2. 四个已核实的脱节

### 2.1 台账的声明范围窄于 README 的权威声明

`docs/integrations/README.md:6-7` 对台账范围的声明是：

> `admissions.tsv` is the machine-readable BR-009 index **for every public Rust
> constant ending in `_ADMITTED`**.

即：台账索引的是**常量**，不是 **Provider、不是 operation、不是数据能力**。

而 `README.md:47-48` 的对外声明是：

> 精确到数据族和 Provider 的当前状态以 准入注册表（`docs/integrations/admissions.tsv`）为准
> （引文出处 `README.md:47-48`；原文此处是一个指向 `docs/integrations/admissions.tsv` 的链接）

以及 `README.md:109`：

> 包含 65 个 RPC，其中 63 个操作有正式准入且可用的 handler

**两个声明之间存在缺口**：一个以"常量"为键，一个以"数据族和 Provider"为键。没有第三个制品把
65 个 operation 逐个映射到它的准入证据。

第二个索引 `docs/integrations/unadmitted-provider-routes.md` 覆盖的是"11 条 repository-unadmitted
Provider×operation registrations exposed by the production gRPC capability service"（`:3-8`），
即**未准入**的那 11 条。**已准入的那 63 条没有任何索引枚举。**

**口径说明（避免过度结论）**：这不是"违反 BR-009"。BR-009（`docs/business_rules.md:154-159`）
管的是"能力广告需有 deterministic 合同测试 + 有界实网探针"。准确的表述是：README 的权威声明
**对 7 个 Provider 不成立**，因为它们的准入状态在台账里没有任何记录。

### 2.2 七个生产接线 crate 没有任何准入门

对全部 42 个 crate 做了 `台账行 × _ADMITTED 常量 × 生产接线` 交叉表。已声明的部分**严丝合缝**：

- 23 个 crate 定义 `_ADMITTED` 常量（60 处 `pub const`，53 个不同名字——`GLOBAL_NEWS_ADMITTED`
  等名字在多个 crate 中重复，这正是台账以 `(crate, constant)` 为键的原因）；
- 60 行台账数据 ↔ 60 处常量定义，一一对应；
- `tools/compliance/check_admissions.py:27-30` 的正则强制"**声明了 `*_ADMITTED` 常量就必须有
  台账行**"（`:166-171` 检查缺失与未知常量）。

缺口在**覆盖范围**，不在执行力。以下 7 个数据源 crate **既无常量、也无台账行、也无
`unadmitted-provider-routes.md` 行，但在生产中被注册为已准入**：

| crate | 生产接线 | 2026-10-07 探针 |
| --- | --- | --- |
| `magic-sina-rs` | `grpc_production.rs:623,977,1190` | **failed** |
| `magic-tencent-rs` | `grpc_production.rs:523,3312-3408,3341` | **failed** |
| `magic-ths-rs` | `grpc_production.rs:2127,2168,2193,2220,2245` | completed |
| `magic-cninfo-rs` | `grpc_production.rs:735,4923` | completed |
| `magic-cls-rs` | `grpc_production.rs:821` | completed |
| `magic-thepaper-rs` | `grpc_production.rs:840` | completed |
| `magic-gov-rs` | `grpc_production.rs:787` | completed |

已核实这 7 个 crate 内部**不存在任何形式的准入门**（grep `ADMITTED|admitted|_ENABLED|_VERIFIED`
只命中字段级"单位/身份未验证"守卫和测试函数名，无门）。

它们的"已准入"状态只存在于两处：
1. `grpc_production.rs` 里硬编码的 `admitted(...)` 调用；
2. `unadmitted-provider-routes.md` **其他行的"替代项"列里的散文提及**——
   `:17` 列 `Tencent`/`Tdx`/`Sina`/`HithinkFinance`/entitled `EmQuant`，
   `:20-21` 列 `Tencent`/`Tdx`/`Sina`/`Szse`，`:22` 列 `Cailianpress`/`ThePaper` 等。

**既不是行，也不是常量，是别人格子里的一个字符串。**

### 2.3 两套并存的常量机制（crate 内自门 vs 仅失败路径生效）

`grpc_production.rs` 里 `_ADMITTED` 常量只在 **17 处**被读取。核查其中每一处的语义后，发现
**两种完全不同的机制**并存：

**机制 A — crate 内自门（常量真正控制成功路径）**：Provider crate 在自己的方法里检查常量。
已核实：

- `magic-hithink-rs`：12 处读取，其中 9 处是真门——`lib.rs:227,530,548,565,582,596`、
  `corporate_actions.rs:52`、`financials.rs:73`、`metadata.rs:76`；
- `magic-imf-rs/src/lib.rs:93`、`magic-stcn-rs/src/lib.rs:145`、
  `magic-tdx-rs/src/company.rs:135`；
- `magic-eastmoney-rs/src/lib.rs:114,121,133`（经 `rankings.all_admitted()`）。

**机制 B — 常量只在 composition 的失败路径上生效**：`grpc_production.rs` 读取常量**只发生在
client 构造失败的 `Err` 分支**里，用来选择报 `runtime_unavailable` 还是 `blocked`；成功路径
一律走 `admitted(...)`。

已验证的两处代码：

```rust
// grpc_production.rs:3270-3280 —— 成功路径无条件真
fn admitted(operation: Operation, provider: &str, exact_scope: &str) -> Capability {
    Capability { operation, repository_admitted: true, runtime_available: true, ... }
}

// grpc_production.rs:1506-1512 —— hithink bars 走 admitted()，不读常量
registry.register_handler(
    admitted(Operation::HistoricalBars, "HithinkFinance", HITHINK_HISTORICAL_BARS_SCOPE), ...)

// grpc_production.rs:1618-1625 —— hithink Auctions 硬编码 blocked()，不读常量
registry.register_diagnostic_handler(
    blocked(Operation::Auctions, "HithinkFinance", HITHINK_AUCTIONS_SCOPE, HITHINK_AUCTIONS_BLOCKER), ...)
```

`HITHINK_AUCTIONS_ADMITTED` 在 `:1469` 被读取——那是在 `:1426` 的 `Err(error) =>` 分支内。
**即该常量的实际语义是"client 构造失败时如何报告"，而不是"该 operation 是否准入"。**
是否准入由**跑哪个注册函数**决定，也就是由硬编码的控制流决定。

`Operation::Auctions` 有两处注册，都**不读该常量**：

- `:1618-1625`（hithink）在 `register_hithink_client` 内，即**成功路径**上无条件
  `register_diagnostic_handler(blocked(...))`；
- `:1288-1293`（miaoxiang）在 client 构造**失败分支**内（blocker 文本以
  `EASTMONEY_API_KEY or MX_APIKEY is not configured` 开头，`:1280,1286,1292` 同一组）。

前者说明：即使 `HITHINK_AUCTIONS_ADMITTED` 为 `true`，只要 client 构造成功，
`Auctions` 仍然被注册为 `blocked`。**把该常量改成 `true` 不会改变任何对外行为。**

反例：EmQuant 是**唯一**一个 composition 在**成功路径**上读常量的例子——
`grpc_production.rs:1762` `if EMQUANT_DAILY_BARS_ADMITTED {` 真的在成功路径上分支
（`else` 分支注册 `blocked` 诊断，`:1772-1780`）。其余常量要么走机制 A（在 crate 内自门），
要么只走机制 B。

**结论**：断言"翻一个 bool 就能上线"对**大部分**常量不成立，只对机制 A 的成立。这直接影响
§6 里"补常量"这一改动的价值评估。

### 2.4 `status` / `last_live_date` 记录的是准入事件，不是当前健康

台账列语义（`docs/integrations/README.md:8-9`）：admitted 行要求证据文档 + canonical
live-evidence 日期 + ≥2 次有界实网探针 + ≥3 次串行负载调用。

仓内最新的实网证据是 `docs/evidence/2026-10-07-upstream-live-probes-230223/summary.json`
（今天 2026-10-14 之前一周）。其 `counts` 为：

```json
{"completed": 28, "failed": 10, "blocked_unadmitted": 1,
 "blocked_unverified_template": 1, "diagnostic_complete_unadmitted": 1,
 "skipped_non_continuous_session": 1}
```

10 个失败探针中，**6 个 crate 在台账里写着 `admitted`**：

| crate | 台账 | 2026-10-07 实网结果 |
| --- | --- | --- |
| `magic-baidu-rs` | admitted (2026-08-17) | failed |
| `magic-tdx-rs` | admitted (2026-08-14) | failed |
| `magic-hithink-rs` | admitted (多条) | failed |
| `magic-yonhap-rs` | admitted (2026-08-16) | failed |
| `magic-emquant-rs` | admitted (2026-08-21) | failed ×2 |
| `magic-eastmoney-rs` | admitted (多条) | failed |

**台账回答"曾经准入过吗"，不回答"现在还能用吗"。** 这不是缺陷，是列语义未声明。

### 2.5 EmQuant：台账与最新证据直接矛盾

- `admissions.tsv:26` — `EMQUANT_DAILY_BARS_ADMITTED  true  admitted  2026-08-21  2  4  -`
- `docs/evidence/2026-10-07-upstream-live-probes-230223/magic-emquant-rs.stderr.log:1` —
  `... EQERR_ACCESS_EXPIRE: the account's EMQuant API entitlement has expired`（5 个族全挂）
- 同目录 `emquant-daily-bars.stdout.log` 为 0 字节

EmQuant 的真实依赖**不是**东方财富桌面终端，而是 **EMQuant SDK + 有效 API 授权**：
`tools/emquant/snapshot_bridge.cpp:240` 调 SDK `start()` 登录；文档明写桌面终端登录
**顶替不了** API 授权（`docs/integrations/eastmoney-emquant.md:105,113`、
`crates/magic-emquant-rs/README.md:45-46`）。

**"EmQuant 是授权阻塞源"这个说法在运行层面成立、在台账层面不成立。**

## 3. `blocked` 桶的分类错误

9 行 `admitted=false` 全部挤在同一个 `status=blocked` 里，但它们的**阻塞类别、成本、责任方**
完全不同：

| 行 | crate | 常量 | 阻塞类别 | 背后有实现吗 |
| --- | --- | --- | --- | --- |
| 4 | cfets | `DR007_ADMITTED` | **真第三方授权** | **无**（`transport.rs` 无 DR007 取数路径） |
| 8 | imf | `ECONOMIC_SERIES_ADMITTED` | 反爬 403 + 门户登录 | 有，完整 |
| 10 | jin10 | `ECONOMIC_CALENDAR_ADMITTED` | **源已下线**（2025-12-01） | 有，但源没了 |
| 19 | stcn | `GLOBAL_NEWS_ADMITTED` | 源归因空白/不安全 | 有，完整 |
| 34 | hithink | `AUCTIONS_ADMITTED` | 源字段不全 | 有，完整诊断 |
| 41 | tdx-local | `LOCAL_TERMINAL_SOURCE_RECORD_COUNT_ADMITTED` | 语义未证明 | **无** |
| 47 | eastmoney | `MX_DAILY_FUND_FLOW_ADMITTED` | 契约不稳定 | 有；常量**运行时不读**（见 §2.3 机制 B） |
| 48 | eastmoney | `MX_OPENING_AUCTION_ADMITTED` | 响应形态漂移 | 同上 |
| 59 | official-news | `GACC_PUBLICATIONS_ADMITTED` | **传输/TLS + 模板未验证** | 半成品 |

**9 个里只有 1 个是真·第三方授权问题。** 归成一类会让人以为要签 9 份合同；实际只有 1 份，
且有 4~5 个的解法是"再探一次 + 补一个解析分支"。

## 4. GACC 的红线与双重阻塞

- blocker 原文（`admissions.tsv:59`）：
  `Shared native Rust request fails before a valid HTTP response; Windows default TLS reports
  name mismatch and chain errors; templates unverified with no TLS bypass`
- 传输用 **OS 信任库 + `rustls-platform-verifier`**，无自定义根证书
  （`crates/magic-market-transport/src/http.rs:464-484`，`Cargo.lock` 有
  `rustls-platform-verifier`、无 `webpki-roots` 生产依赖）
- GACC 模板的 `rows`/`body`/`title` 选择器**是空的**
  （`crates/magic-official-news-rs/src/profiles.rs:27`）

**两条独立结论**：

1. blocker 文本与 `AGENTS.md` 的 HTTP/TLS 政策**同时禁止绕过 TLS 校验**。任何解法不得削弱
   证书验证、不得加异常、不得改信任库。这一点必须写进任何后续 GACC 工作项。
2. **即使解决了 TLS，GACC 仍然跑不通**——`crates/magic-official-news-rs/src/lib.rs:250-254`
   在 `profile.rows.is_empty()` 时返回
   `Unsupported("current dynamic listing requires a separately verified profile")`。
   2026-10-07 的诊断探针实测就是这个结果（`official-gacc-diagnostic.stderr.log`）。
   **模板未验证与传输失败是两个独立阻塞，当前挤在台账的一个 `blocker` 单元格里。**

## 5. 附带发现（第六个问题的回响）

2026-10-07 `magic-eastmoney-rs` 探针失败的两个族（`summary.json.failed_family_lines`）：

```
dragon_tiger.discovery: core contract error: invalid request: dragon-tiger net amount must equal buy amount minus sell amount
dragon_tiger.market:    core contract error: invalid request: dragon-tiger net amount must equal buy amount minus sell amount
```

`dragon_tiger.discovery` 是第六个问题里**含可转债**的那个 op（`discovery.rs:11-19` 只按
`TRADE_DATE` 过滤，`:128` 有 `060 => AssetClass::Bond` 分支）。它在实网下**整条失败**——源返回的
净额 ≠ 买入 − 卖出，Core 的 `DragonTigerEntry` 构造校验直接拒。

它**没有台账行、没有常量**（`magic-eastmoney-rs` 的 6 个行是 MX_* 与 fund_flow/rankings/post_close）。

## 6. 拟议修改（待 §7 拍板）

本设计把改动分成两类，**价值与风险差异显著，批准时应拆开**（见 §11）。

### 6.1 纯账本修复（零运行时行为变更）

1. **台账列语义声明**：在 `docs/integrations/README.md` 明确 `status`/`last_live_date` 记录的是
   **准入事件**，不是当前健康；并指向仓内最新的实网证据批次。
2. **阻塞分类**：`admissions.tsv` 增加 `blocker_class` 列（D3）。
3. **EmQuant 行修正**（D5）：使该行与 2026-10-07 证据一致。
4. **GACC 行的双重阻塞显式化**（D6）：在同一 `blocker` 单元格内区分"传输失败"与"模板未验证"，
   并写入"不得绕过 TLS"的红线。
5. **7 个 crate 的处置**（D2）：或补常量+行，或在文档中显式声明其不属于 BR-009 常量约定，
   并在 `unadmitted-provider-routes.md` 中显式列出。

### 6.2 有运行时行为变更的修复

6. **补 `_ADMITTED` 常量并加门**（D2 选 (a) 时）：为 7 个 crate 各补常量，并在其 crate 内加
   机制 A 的门。**注意**：新增门意味着某些当前对外可用的 operation 会变成
   `Unsupported`，直到探针通过——这是对外可见的行为变更，不是纯账本工作。

## 7. 待批准人拍板（按编号回答）

- **D1 — 台账的声明范围**：(a) 保持"仅 `_ADMITTED` 常量"，另立一个 provider 级索引覆盖
  7 个 crate；(b) 把 `admissions.tsv` 扩到"每个生产注册的 Provider×operation"，常量降为
  可选列。**(b) 更彻底但改动台账 schema**，会影响 `check_admissions.py` 与
  `unadmitted-provider-routes.md` 的生成关系。
- **D2 — 7 个 crate 的处置**：(a) 补 `_ADMITTED` 常量 + 机制 A 的门 + 台账行（与现有 23 crate
  一致，但**有对外行为变更**）；(b) 只在文档中声明其为"BR-009 之前的历史接入，不适用常量约定"，
  并在两个索引中显式列出（**零行为变更**）；(c) 两者都做，先 (b) 后 (a)。
- **D3 — 阻塞分类**：(a) 增加 `blocker_class` 枚举列；(b) 保持自由文本，仅在
  `unadmitted-provider-routes.md` 里分类。
- **D4 — 健康 vs 准入**：(a) 保持 `status` 为准入事件，另加 `last_probe_utc` /
  `last_probe_status` 两列，由探针 summary 回填；(b) 只改文档措辞；(c) 不动。
- **D5 — EmQuant 行**：(a) 改为 `admitted=false` + 授权过期 blocker；(b) 保持 `true`
  （当前设计用运行时 `runtime_available=false` 表达）；(c) 保持 `true`，但在 blocker/备注列
  记录 2026-10-07 授权过期。
- **D6 — GACC**：(a) 仅在 evidence 文档记录"不得绕过 TLS"+"模板须先验证"；(b) 同时在
  台账同一单元格内显式区分两个阻塞（**不拆行**——拆行会破坏 `(crate, constant)` 唯一键）。

## 8. 被否决的替代方案

- **让 `Capability.repository_admitted` 从台账派生**：需要 composition 在构建期读
  `admissions.tsv`，引入构建期文件依赖与解析失败模式，而台账本身不授予数据权利
  （`docs/integrations/README.md:21-24` 明确"不执行探针、不授予数据权利"）。改为
  **加一个离线一致性检查**（§6.1.5 的表述方式），代价低一个数量级。否决构建期派生。
- **把 `unadmitted-provider-routes.md` 改为机器生成**：该文档 `:56-57` 明确"copied into the
  client bundle"，改动它会进入客户端 bundle 契约。超出本设计范围。记录为后续项。
- **为了对齐而重新探针**：本设计不执行任何实网探针。台账的 admitted 行要求 ≥2 次有界探针，
  但那是**准入时刻**的要求，不是持续健康监测。把两者混起来会扩大范围。

## 9. 影响面与风险

- **爆炸半径**：若全部选 (b)/(c) 方向，改动仅限 `admissions.tsv`、
  `docs/integrations/README.md`、`unadmitted-provider-routes.md`、
  `tools/compliance/check_admissions.py`（若 D3/D4 加列）。**proto、`contract_sha256`、
  `grpc_production.rs` 零变化。**
- **若 D2 选 (a)**：新增门会使当前对外可用的 operation 变成 `Unsupported`。**这是对外可见的
  能力收缩**，必须确认下游消费者（Mac 侧 bundle 使用者）是否依赖这些 operation 当前可用。
- **对 Gate C 的影响**：`admissions.tsv` 加列会改变
  `tools/compliance/test_check_admissions.py` 的期望，需同步。本仓无 PowerShell 7，
  `tools/docs/check_links.ps1` 不可运行，文档链接检查需人工完成。
- **负价值边界**：修改 EmQuant 行**不会让它变得可用**。本设计不授予数据权利、不恢复授权、
  不改变任何 provider 的运行时行为（除非 D2 选 (a)）。

## 10. 未验证项（不写成结论）

- 未运行任何实网探针。2026-10-07 批次是仓内最新证据，但**它是单次运行**，且其
  `scope_limits` 自述"One live run per planned path, no serial-load or release-gate
  certification"与"An early failing stage may prevent later families in the same executable
  from running"。因此 §2.4 的 6 个 admitted-but-failed 行**不能**推断为持续故障。
- 未确认下游是否有消费者依赖 7 个 crate 当前的 operation 可用性（D2 的风险前提）。
- 未确认 `README.md:109` 的"63 个操作有正式准入"如何计数；两个索引都无法复现该数字。
- IMF 的 Akamai 403 未验证是 UA 特定还是 IP 特定（证据只有 403 状态码）。
- GACC 的 TLS 失败未验证是否与 SNI 有关（仓内无此声明，本设计不主张）。

## 11. 价值评估与拆分建议（2026-10-14 补充）

### 11.1 零运行时风险的账本修复（确定收益）

D1 文档对齐 + D3 阻塞分类 + D4 列语义 + **D5 EmQuant 行修正** + D6 GACC 双重阻塞显式化 +
D2 选 (b)。

修的是 §2 的五处脱节：README 权威声明对 7 个 Provider 不成立、`blocked` 桶掩盖了完全不同的
成本结构、台账列语义未声明、EmQuant 行与最新证据矛盾、GACC 的两个阻塞挤在一个单元格里。

这五条**即使永远不补常量、不加门也照样成立**，且几乎不涉及运行时逻辑。

其中 §2.4 与 §2.5 是**活陷阱**：把台账 `admitted` 当作"现在能用"会直接导致调用失败。

### 11.2 需要工程改动（有前提）

D2 选 (a)：为 7 个 crate 补常量与机制 A 的门。

价值取决于**下游是否依赖这些 operation 当前可用**。若依赖，这是能力收缩，需要先协调；
若不依赖，补门能让 README 的权威声明重新成立。**在确认消费方之前不做。**

### 11.3 明确不做的

- 不重新探针（探针是准入时刻的证据，不是持续监测）。
- 不改 TLS 策略、不加证书异常（§4）。
- 不改 proto / `contract_sha256` / 运行时注册逻辑（除非 D2 选 (a)）。
- 不修 §5 的 dragon-tiger 实网失败——那是独立缺陷，不是准入治理问题，应单独立项。
