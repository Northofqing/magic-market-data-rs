# Gate A 设计：证券主数据字段填充分级（2026-10-14）

状态：待批准（决策点见 §7，按编号拍板后进入 Gate B）
问题 op：`SecurityProfiles`、`SecurityMetadata`（拟不新增 op）

## 1. 问题与范围

`SecurityProfile`（`crates/magic-market-core/src/company.rs:14-24`）的 8 个字段中，
`industry` / `total_shares` / `floating_shares` **从未被任何 provider 填充**；
`SecurityMetadata`（`crates/magic-market-core/src/provider.rs:975-991`）的
`price_limit` / `status` 同样从未被真实填充。本设计逐项判定**每一项的真实阻塞**，
并区分三类性质完全不同的改动。

**明确不在本设计范围**（各自独立）：

- **宇宙枚举**（"完整已上市证券清单"）：无 op、无合同。TDX 有实现
  （`magic-tdx-rs/src/service/mod.rs:45-74` `security_list_all_with`，带声明数原子校验）
  但生产调用点为零，且 `SecurityInfo` 只有 5 个字段
  （`protocol/types.rs:103-110`：`code/volunit/decimal_point/name/pre_close`）。
  **没有退市记录的宇宙 op 是净负资产**（§9.3），必须先有退市源。
- **退市 / 上市状态生命周期**：合同层不存在。`lifecycle.rs` 是公司行为，
  `CorporateActionStatus`（`lifecycle.rs:34-41`）是**行为的**状态；
  `business_rules.md:1007-1008` 明文 "Contract expiry is not equity delisting"。
  属新数据源准入。

## 2. 字段现状全表（逐行核实）

### 2.1 `SecurityProfile` — 唯一实现是 TDX

| 字段 | 行 | 状态 | 出处 |
| --- | --- | --- | --- |
| `instrument` | `:16` | TDX 填充 | `magic-tdx-rs/src/company.rs:176` |
| `name` | `:17` | TDX 填充 | `company.rs:177` |
| `industry` | `:18` | **恒 `None`** | `company.rs:178`；**测试钉死** `company.rs:311` |
| `listed_on` | `:19` | TDX 填充（finance `ipo_date`） | `company.rs:179`、`adapter.rs:1789-1800` |
| `total_shares` | `:20` | **恒 `None`** | `company.rs:180`；**测试钉死** `company.rs:312` |
| `floating_shares` | `:21` | **恒 `None`** | `company.rs:181` |
| `facts` | `:22` | TDX 填充（F10 原文行） | `company.rs:174,182`、`:189-213` |
| `evidence` | `:23` | TDX 填充 | `company.rs:183` |

`SecurityProfiles` trait 只有一个生产实现（`magic-tdx-rs/src/company.rs:128`），
上限 8 个标的（`company.rs:21,215-220`）。

### 2.2 `SecurityMetadata` — 四个实现（EmQuant 不构造）

| 字段 | 行 | TDX | Sina | Tencent | Hithink | EmQuant |
| --- | --- | --- | --- | --- | --- | --- |
| `name` | `:980` | `adapter.rs:1827` | `lib.rs:902` | `lib.rs:1138` | `metadata.rs:215` | — |
| `board` | `:981` | 推导 `:1828` | 推导 `:903` | 推导 `:1139` | `None :216` | — |
| `is_st` | `:982` | 名字推导 `:1802-1803` | 名字推导 `:904` | 名字推导 `:1140` | `None :217` | — |
| `listed_on` | `:984` | `:1830` | `None :905` | `None :1141` | `None :218` | — |
| `price_limit` | `:985` | `None,None :1831` | `:906` | `:1142` | `:219` | — |
| `status` | `:986` | `Unavailable :1832` | `:907` | `:1143` | `:220` | — |
| `source_at` | `:987` | `None :1833` | `:908` | `:1144` | `:221` | — |

- **`price_limit` 从未有真实值**：全仓 `PriceLimitRule::new(...)` 的生产调用点
  （`sina/lib.rs:906`、`tencent/lib.rs:1142`、`tdx/adapter.rs:1831`、`hithink/metadata.rs:219`）
  **参数全是 `None, None`**；带真实参数的调用只存在于测试（`core/tests/serde_contracts.rs:168`、`capital.rs:291`）。
- **`status` 恒为 `DataStatus::Unavailable`**：四个 builder 全部如此
  （`tdx/adapter.rs:1832`、`sina/lib.rs:907`、`tencent/lib.rs:1143`、`hithink/metadata.rs:220`）。
- **EmQuant 根本不构造该结构**：`magic-emquant-rs/src/lib.rs:1094-1096` 直接
  `Err(EmQuantError::Unsupported("security master fields and their source timestamps are not verified in EMQuant"))`。

## 3. `listed_on`：源发布了、provider 校验了、然后丢弃

`magic-hithink-rs/src/metadata.rs` 的原生字段是完整解析并校验的：

- `list_date` / `end_date` / `last_trade_date` / `last_delivery_date`（`:102-110`）
- 每个都校验为规范公历日期，含 `0000-` 前缀拒绝（`:131-151`）
- **对每一条返回行都校验**，包括模糊/不匹配行（`:184-187`）

但构造记录时（`:213-225`）`listed_on` 传 `None`（`:218`）。**这不是数据缺失，是策略**，
理由写在 `business_rules.md:1006-1009`。

该理由需要拆开看，因为它对四个字段的效力不同：

| 规则原句 | 适用字段 | 对 `list_date` 是否成立 |
| --- | --- | --- |
| "Contract expiry is not equity delisting" | `end_date`/`last_trade_date`（期货字段） | **不成立**——上市日期在任何资产类别里都是上市日期 |
| "the current code-table loading time is not historical fact availability" | `list_date` | **部分成立**——ticker-search 返回**当前**代码表 |

规则还把四个字段与 "Board, ST state, Core `listed_on` and price-limit rules stay absent"
（`:1009-1010`）合并陈述，但四者的阻塞并不相同。**这是 §7 D2 需要拍板的原因。**

TDX 侧 `listed_on` **已填充**（`company.rs:179` ← `adapter.rs:1789-1800` 读 finance `ipo_date`），
所以"导出上市日期"在本仓已有先例，Hithink 是唯一扣下的一项。

## 4. 股本：数据在同一包里，但**单位从未被本仓声称过**

### 4.1 数据确实在手

`magic-tdx-rs/src/protocol/parsers.rs:1103-1156` 从 finance 包解出两个字段：

- `liutongguben`（流通股本）在 `:1106-1107` —— 包的开头就是它
- `zongguben`（总股本）= `fields[0]` 在 `:1156`
- 两者都有 finite 校验（`:1134-1138`、`:1142-1146`）
- `FinanceInfo` 结构确实携带两者（`protocol/types.rs:147,152`）

**而安全主数据路径（`adapter.rs:1789-1800`）取的就是这个包，只读了 `ipo_date`。**

### 4.2 但**不能**接进 `Quantity`

parser 自己的注释写明了原因：

- `parsers.rs:1105` — "TDX 原始值（**单位不固定，由用户自行判断**）"
- `parsers.rs:1129` — "全部返回 TDX 原始值，**不做单位转换**"

而 `core/src/lifecycle.rs:26-32` 已定义了这个情形的类型：

> `UnverifiedSourceUnit::ProviderNative` — "A provider-native quantity whose physical unit
> has not been independently verified. **Consumers must not interpret values carrying this
> unit as shares**, lots, or a per-share ratio."

**`SecurityProfile.total_shares: Option<Quantity>` 不是那个类型。** `Quantity` 意味着单位已验证。

### 4.3 证据：本仓从未在 Core 边界声称过任何已验证的股本单位

唯一另一条股本路径是 XDXR → `CorporateActionTerms::CapitalStructure`
（`adapter.rs:2053-2058` 用 `qianzongguben`/`houzongguben`），而该变体**同样携带
`UnverifiedSourceUnit`**（`lifecycle.rs:63-70` 的 `unit: UnverifiedSourceUnit`）。

且 `parsers.rs:1288-1297` 对其施加的 `get_volume(...)` **不是单位换算**，
而是 TDX varint **解码**（`helpers.rs:20-34`："交易量解码 - 对应 Python tdxpy helper.py
中的 get_volume 函数"，做的是变长整数解包）。解码之后单位仍是 TDX 原生。

**即：两条路径都止步于 `UnverifiedSourceUnit`。把 finance 包的 `zongguben` 接进
`Quantity`，会成为本仓第一处声称"已验证股数单位"的地方。** 这就是 §7 D1 的实质。

## 5. `industry`：结构化的行业事实以**非结构化文本**存在于 `facts`

`magic-tdx-rs/src/company.rs:296-317` 的 F10 测试夹具内容是：

```
" 公司名称  华电辽宁能源发展股份有限公司\n所属行业 电力 \n"
```

即 F10 的 company-overview 段**字面包含 `所属行业 电力`**。它被
`profile_facts`（`company.rs:189-213`）转成一个 `ProfileFact`——测试在 `:313-317` 断言
`facts[0].value == "公司名称 华电辽宁能源发展股份有限公司"`——而结构化的
`industry` 槽位在同一测试里被断言为 `None`（`:311`）。

同时 `FinanceInfo` 携带一个**数值** `industry: u16`（`protocol/types.rs:149`），
与 `province: u16` 一起被同一路径丢弃（全 crate 无 `.industry` 读取点）。

**所以 `industry` 有两份来源（F10 文本行、finance 数值码），两份都未映射到结构化槽位。
这是映射决策，不是数据缺失。** 但两条路都各有一个硬问题：F10 行是自由文本
（切分规则未定义），finance 码是数字（需要 code→name 对照表，且该表本身是权威性问题）。

## 6. `board` / `is_st`：推导，且**已声明**

TDX 的 `st_flag()`（`adapter.rs:1644-1656`）检查证券简称是否以 `ST`/`*ST`/`S*ST`/`SST` 开头；
Sina（`lib.rs:868-874`）、Tencent（`lib.rs:1104-1110`）同法。`board` 三家均由
exchange+code 前缀推导。

**这不是静默伪造**：TDX 在 `issues` 里逐条声明了推导与缺失，例如
`adapter.rs:1810-1813` — "{code}: board is derived from exchange/code because the TDX
list packet has no board field"，以及 `:1814-1816`（缺上市日期）、`:1817-1820`（缺涨跌停规则）。
这与本仓"显式优于静默"的纪律一致，**应予保留，不在本设计内改动**，除非 D5 另有决定。

**真正的缺口在别处**：Hithink 有限价池条目（limit-up pool）的**源端真 `is_st` boolean**，
被显式丢弃——`magic-hithink-rs/src/lib.rs:1274`：

```rust
let _source_flags = (item.is_st, item.is_new);
```

下划线绑定直接扔掉。这是**源端提供的真值**，比名字前缀推导强。要接它需给
`LimitPoolEntry` 加字段（`:1275-1279` 构造点），属小改动，且不触碰 `SecurityMetadata`。

## 7. 待批准人拍板（按编号回答）

- **D1 — 股本字段的合同形态**（本设计的核心）。三选一：
  - (a) **改合同类型**：让 `total_shares`/`floating_shares` 携带 `UnverifiedSourceUnit`
    （仿 `lifecycle.rs:58-62` 的 `ProviderNativeRatio` 形态），从而可以接线而不违反单位纪律；
  - (b) **先证明单位**：完成对 TDX finance 股本单位的有界实网证明，再填 `Quantity`；
  - (c) **保持 `None`**，并在合同文档写明"本仓不提供已验证股数"。
  **推荐 (a)**：(b) 需要一次实网证明任务且 TDX 非权威源；(c) 放弃一个高价值字段；
  (a) 与仓库既有类型一致且立刻可用。
- **D2 — `listed_on` 的 Hithink 放行**：(a) 放开 Hithink `list_date`（需同时明确
  §3 表格里"current code-table"那句对它的实际约束）；或 (b) 维持现状。
  **需先回答一个事实问题**：当初把 `list_date` 与三个期货字段合并扣下，是因为
  `list_date` 本身有问题，还是因为合并处理更省事。
- **D3 — `industry` 结构化**：(a) 从 F10 `所属行业` 解析（需定义切分规则的稳定性证据）；
  (b) 从 finance `industry: u16` 映射（需 code→name 权威对照表）；(c) 维持现状
  （留在 `facts` 非结构化）。**推荐 (c) 或延后**：两条路都引入新的权威性依赖。
- **D4 — Hithink 限价池 `is_st` 真值**：(a) 给 `LimitPoolEntry` 加字段承载；
  (b) 维持丢弃。**推荐 (a)**：低成本，且它比名字推导可靠。
- **D5 — `board`/`is_st` 推导**：确认维持"推导 + `issues` 声明"现状
  （本设计不改），或要求引入源端字段。
- **D6 — 记录 schema 版本策略**：本设计全部为**记录侧新增可选字段**，不触碰请求侧
  （`InstrumentsRequest` 带 `#[serde(deny_unknown_fields)]`，`grpc_production.rs:338-342`，
  不得改动）。需确认走 per-family 版本常量升级——先例：`FINANCIAL_STATEMENTS_SCHEMA_VERSION = 2`
  （`grpc_production.rs:99`，同为 `company.rs` 族）、`NEWS_SCHEMA_VERSION = 2`（`:97`），
  而 `SecurityProfiles`/`SecurityMetadata` 目前使用全局 `SCHEMA_VERSION = 1`（`:96`）。

## 8. 被否决的替代方案

- **把 `zongguben`/`liutongguben` 直接接进 `Quantity`**：违反
  `lifecycle.rs:26-32` 的 `UnverifiedSourceUnit` 纪律，成为本仓第一处声称已验证股数单位
  的地方。否决（D1 选 (a) 或 (b) 才可接线）。
- **用名字前缀推导当作 `is_st` 的真值来源**：已存在且已声明，但它是推导。仅作为
  无真值时的降级，不得反过来阻止 D4 接入 Hithink 真值。否决把它当作终态。
- **新增宇宙 op 来"顺带"解决主数据**：无退市记录 → 幸存者偏差（§9.3）。否决。
- **新增 op 承载字段**：本族已是 JSON 记录，字段是记录侧新增，无需 proto/enum 变更。否决。
- **放宽 `status: DataStatus::Unavailable`**：它是当前正确的诚实取值（身份批次完整
  但记录数据不可用，见 `business_rules.md:1010-1011`）。在 D1–D4 真正填充字段之前
  放宽它就是伪造可用性。否决。

## 9. 影响面与风险

### 9.1 爆炸半径

- **记录侧新增可选字段**：`SecurityProfile`（Core）+ 各 provider 映射 + 记录 schema 版本常量。
- **请求侧零改动**（`deny_unknown_fields`，不动）。
- **proto / `contract_sha256` 零改动**（两个 op 的载荷都是 `CanonicalPayload` JSON）。
- **协调成本**：记录 schema 版本升级需重建 client bundle 并与对端确认其消费端不做
  `deny_unknown_fields`。**这是本设计的主要协调成本**，量级低于新增 op（gap #2）但非零。

### 9.2 语义风险

- **D1(a) 的类型选择若被下游误读**：携带 `UnverifiedSourceUnit` 的字段必须让调用方
  无法把它当股数用。若 C 端或下游某处按数量直接算市值/换手率，会把未验证单位
  当已验证用。**须在 op 描述与合同文档写明"不得用于市值、换手率、股本比例计算"。**
- **D2 放行 `list_date` 的前视风险**：ticker-search 是**当前**代码表，已退市标的无行。
  由此得到的上市日期集合**天然不含退市标的**，用于历史宇宙即幸存者偏差。
- **D3(a) F10 文本切分的稳定性**：自由文本切分规则若无来源稳定性证据，
  会成为新的静默失败面。这是推荐延后的原因。

### 9.3 与宇宙 op 的次序约束（必须记录）

**本设计的任何字段填充都不会消除宇宙层的幸存者偏差。** 没有退市记录时，
下游拿"当前快照 + 推导 ST"构造历史宇宙，会同时产生幸存者偏差与 ST 状态前视偏差，
且失败是静默的——与本仓 `magic-hithink-rs/src/historical_coverage.rs:44-49`
自述 `pit_guarantee: false` 属同一类风险。因此**若未来接入宇宙 op，其 op 描述必须
写明"当前快照、无退市记录、不得用于历史宇宙"**。

### 9.4 Gate C 提醒

本仓无 PowerShell 7，`tools/docs/check_links.ps1` 不可运行，文档链接检查需人工完成。

## 10. 未验证项（不写成结论）

- 未运行实网探针。TDX finance 股本的**实际单位**（惯例为"万股"）未被本设计证明，
  静态追踪无法证明（D1(b) 需要这一步）。
- 未确认下游/client bundle 消费端是否对记录使用 `deny_unknown_fields`（D6 的前提）。
- 未确认原规则作者把 Hithink `list_date` 与三个期货字段合并扣下的具体理由（D2 的前提）。
- 未确认 F10 段落 `所属行业` 的切分是否在所有标的与所有 F10 版本上稳定（D3(a) 的前提）。
- 未核实 Sina/Tencent 的 `board` 推导函数细节（仅确认其为推导并记录在 `issues`）。

## 11. 价值评估与拆分建议

### 11.1 分级

| 级 | 内容 | 前提 | 建议 |
| --- | --- | --- | --- |
| **T1** | D4：Hithink 限价池 `is_st` 真值 | 无 | 独立小改动，先做 |
| **T2** | D1：股本字段合同形态 | 需 D1 拍板 | 价值最高（数据已在同一次调用中） |
| **T3** | D2：`listed_on` Hithink 放行 | 需先回答规则意图 | 低改动，但前提未清 |
| **T4** | D3：`industry` 结构化 | 需新权威性依赖 | 建议延后 |
| **T5** | 宇宙 op / 退市源 | 新数据源准入 | **独立项目，且必须先于宇宙 op** |

### 11.2 为什么 D1 值得先于其他字段

- 数据**已在安全主数据调用已取回的包里**（§4.1），零新增网络 I/O、零新增来源。
- 它同时是一个**证据纪律的活口子**：只要有人图省事把 `zongguben` 接进 `Quantity`，
  就绕过了 `UnverifiedSourceUnit` 这道设计。把类型定对，等于同时修好一个未来陷阱。
- `listed_on` 的前提未清、`industry` 需新依赖、`is_st` 价值面窄——三者都不如 D1 干净。

### 11.3 明确不做

- 不做"顺带接出宇宙列表"。见 §9.3。
- 不把任何推导值提升为源端事实。§6 的 `issues` 声明机制是正确形态，应保留而非替换。
