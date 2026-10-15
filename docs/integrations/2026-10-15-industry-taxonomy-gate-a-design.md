# Gate A 设计：行业分类体系缺失（申万 / 中信）（2026-10-15）

状态：待批准（决策点见 §7，按编号拍板后进入 Gate B）

问题 op：**本设计原则上不新增 op。** 缺的不是取数通道——行业数据有 4 条通道在生产环境运行——缺的是**分类体系本身**：权威归属、稳定码、层级、版次与生效日期。仅当 D2 选 (b) 时才需要一个新 op。

---

## 1. 问题

"行业分类缺"有四种互斥解释，核实结果是第四种：

| 假设 | 核实结果 |
|---|---|
| (a) 没有行业数据 | **不成立。** 4 条通道在生产环境运行（§2.1） |
| (b) 行业字段是空的 | **部分成立，但其中一半是文档化的有意选择**（§3.1） |
| (c) 有数据，但分不清是哪家体系 | **成立，且无规则覆盖** |
| (d) **没有权威体系，也没有版次** | **成立。这是主体缺口**（§4） |

本设计的结论：**本仓有 4 套互不兼容的行业身份，没有一套是 申万 / 中信，也没有任何一套带生效日期。**

证据一：仓库全局搜 `申万`，**只命中 1 次**，而且是用来声明"我不是它"——

> `magic-tdx-rs/src/block/mod.rs:18` — `| block_fg.dat | 行业板块 | 155 | 通达信板块标签 (非中证/申万) |`

证据二：`docs/` 全目录搜 `申万|中信|行业分类` → **0 命中**。唯一沾边的是 §3.3 引的那句"TDX 并非指数成分权威源"。

这与第 10 个问题（新闻）**形状相反**：新闻的薄是**规则强制的边界**（BR-012/020/030/031/042/066 逐条钉死）；行业分类的缺是**真的缺**，而且缺在最贵的地方——**没有版次 ⇒ 用今天的行业归属回测就是前视偏差**（§4.iii）。

## 2. 今天的实际覆盖（逐项核实）

### 2.1 四条在生产环境运行的行业通道

| 通道 | 身份形态 | 谁的分类 | 注册/实现位置 | 上线? |
|---|---|---|---|---|
| TDX `block_fg.dat` → `BoardDirectory` / `BoardConstituents` / `BoardMemberships`（`category=Industry`） | `tdx:industry:{名称}` —— **名称做键，无数字码** | 通达信自有 **155** 个 | `board_provider.rs:28,207-238`；注册 `grpc_production.rs:2613-2667` | ✅ `admitted` |
| Eastmoney `BoardFlows`（`category=Industry` → `m:90+t:2`） | `f12` = 东财 BK 码，`f14` = 名称 | 东财自有 | `board_flow.rs:24-33,188-190` | ✅ `admitted` |
| Eastmoney `LimitPools.industry` ← `hybk` | **自由文本**（`NonEmptyText`） | 东财板块名 | `limit_pool.rs:209` | ✅ `admitted` |
| Eastmoney `ResearchReports.industry_code` ← `indvInduCode` | **东财数字码**，且用于严格过滤 | 东财自有 | `reports.rs:458-483,496-497` | ✅ `admitted` |

三条通道的 `board_code` / `industry` 在契约里都是**自由文本**：`BoardDefinition { board_code: NonEmptyText, board_name: NonEmptyText, category: BoardCategory, .. }`（`magic-market-core/src/discovery.rs:12-18`），`BoardConstituentRequest { board_code: NonEmptyText, .. }`（`:187-190`）。**没有任何码空间、命名空间或权威归属字段。**

`BoardDirectory(category=Industry)` 是**可达的**：`BoardDirectoryRequest::new(category, limit)` 不校验取值，composition 侧也不预校验（`grpc_production.rs:2626` 直接透传），provider 侧 `block_type()` 把 `Industry` 映射到 `block_fg.dat`（`board_provider.rs:207-215`）。

### 2.2 解析后被丢弃的两条通道

| 来源 | 形态 | 现状 |
|---|---|---|
| TDX `FinanceInfo.industry: u16` | 通达信协议**数字码** | 解析于 `protocol/parsers.rs:1114-1115,1153`，全 crate **无 `.industry` 读取点** |
| TDX F10 `所属行业` 文本行 | 文本 | 解析器认这些标签（`profile/parser_f10.rs:121`：`["行业类别", "所属行业", "通达信研究行业"]`），测试断言 `parse_basic_info` 能取出 `制造业`（`:319`）；但 `SecurityProfile` 归一化路径**不走这个函数** |

`normalize_profiles`（`magic-tdx-rs/src/company.rs:152-187`）走的是 `profile_facts`（`:189-206`），它把 `公司概况` 段的每一行变成**通用** `ProfileFact { key: "company_overview_line_{:03}" }`，不做语义解释，并在 `:178` 写死 `industry: None`。

### 2.3 `SecurityProfile.industry` 是空的

| 项 | 现状 |
|---|---|
| 契约 | `pub industry: Option<NonEmptyText>`（`magic-market-core/src/company.rs:18`） |
| TDX | `industry: None`（`company.rs:178`），**测试钉死** `assert!(profile.industry.is_none())`（`:311`） |
| Hithink | `industry: None` ×3（`magic-hithink-rs/src/lib.rs:1288,1320,1351`） |
| THS | `industry: None`（`magic-ths-rs/src/lib.rs:600`） |
| 全仓 | **无任何 `industry: Some(...)`** —— 三个 provider 全部写 `None` |

### 2.4 存在的行业形状的契约

| 类型 | 位置 | 说明 |
|---|---|---|
| `SecurityProfile.industry` | `company.rs:18` | 单个自由文本，恒 `None` |
| `LimitPoolEntry.industry` | `magic-market-core/src/limit_pool.rs:29` | 自由文本；**东财真的填了**（`hybk`） |
| `ResearchSubject::Industry(NonEmptyText)` | `magic-market-core/src/research.rs:12` | 研报主体的行业；`ResearchReport.industry_code` / `industry_name`（`:126-127`） |
| `BoardCategory::{Industry,Concept,Region,Unknown}` | `signals.rs:8-15` | **4 个变体，无层级** |
| `MarketRankingKind::{Industry,Concept,Region}` | `signals.rs:464-473` | **结构性不可达**，见 §5.1 |

## 3. 边界与既有裁定：哪些已经定过，不要重复做

### 3.1 `industry: None` 是文档化的有意选择，不是遗漏

`docs/integrations/tdx-public-security-profile.md:14-19` 明写：

> "Every non-empty source line in that section becomes an ordered `ProfileFact`. Whitespace is normalized, but **labels and text are not interpreted as industry**, share counts or another semantic field. `industry`, `total_shares` and `floating_shares` therefore remain `None`."

并且该文档 `:59-61` 划了范围："This admission does not cover Beijing, arbitrary F10 sections, **inferred fundamentals**, realtime freshness or a redistribution right…"

⇒ **禁止从文本推断行业，是已记录的纪律。** 这一条与新闻那次同形，本设计**不推翻它**。`docs/integrations/grpc-external-api.md:459` 同样写着"不推断行业、总股本或…"，`tonghuashun-web.md:34` 也记录了 THS 不提供行业来源。

### 3.2 它 ≠ `security-master-fields` 的 D3

那份设计（`2026-10-14-security-master-fields-gate-a-design.md`）§5 已经写过 `industry`，并给了 D3（`:183-184`）：

> (a) 从 F10 `所属行业` 解析；(b) 从 finance `industry: u16` 映射；**(c) 维持现状**

它的结论 `:259` 是"**D3：`industry` 结构化 | 需新权威性依赖 | 建议延后**"。

**关键：D3 的两个候选源都是通达信自有分类。D3 即使落地，也填不出申万。** 那份设计自己也点破了难点——D3(b) "需 code→name **权威对照表**"（`:184`）。本设计回答的正是"那张表是谁的"。

⇒ **D3 ≠ 本缺口，二者不互相替代**：D3 是"填不填这个槽位"，本设计是"槽位里放谁的体系"。

### 3.3 它 ≠ `tdx-board-zs` 的 D4，但继承它的权威原则

`tdx-board-zs` 的 D4 管的是 `board_code` 的**身份格式**（`tdx:industry:<名称>` vs BR-026 的 `tdx:<文件名>:<名称>`，见该文档 `:33-46`、`:143-146`）。**D4 拍完，行业分类该缺还是缺**——身份格式不是分类体系。

但它裁定过一条**直接适用于本设计**的原则（`:218`）：

> "把 `block_zs.dat` 当作指数数据源。**TDX 并非指数成分权威源。正确解法是中证指数**…"

**对称适用：TDX / 东财 / 同花顺都不是 申万 / 中信 分类的权威源。** 申万分类的权威源是申万宏源，中证行业分类的权威源是中证指数公司。拿它们的行业标签冒充申万，正是那份设计已经否决过的同一类错误。

## 4. 三个真实缺口

| # | 缺什么 | 后果 | 代价 |
|---|---|---|---|
| **i** | **权威体系与稳定码。** 4 套身份互不兼容：通达信名称键（`tdx:industry:{名称}`）、东财 BK 码、通达信协议 u16、东财文本名。`board_code` 是自由文本，无码空间/命名空间/权威字段 | 无法跨源对齐；无法与外部研报、指数权重、卖方观点对齐；名称变更即身份变更 | 中 |
| **ii** | **层级。** `BoardCategory` 只有 `Industry/Concept/Region/Unknown` 四个变体（`signals.rs:8-15`），**没有 一级/二级/三级**，也没有 parent 指针 | 做不了行业聚合、行业中性化、行业因子暴露、行业轮动 | 中 |
| **iii** | **版次与生效日期（PIT）。** 所有通道都是"当前快照"——TDX 是**文件快照且无日期**，东财是实时接口。**全仓没有任何 taxonomy 生效日期/版次机制** | **用今天的行业归属回测某个历史日期 = 前视偏差。** 申万会调整归属与成分，这是硬伤 | **高** |

第 iii 条是本设计最贵的一条，也是六个/十一个设计里**唯一直接命中前视偏差**的一项——与清单第 4 项（PIT / 修订 / 完整性证据，`pit_guarantee: false`）是同一个病根的两个面。

## 5. 附带发现

### 5.1 `MarketRankingKind::{Industry, Concept, Region}` 是结构性死变体

`MarketRankingKind` 声明了 7 个变体，含 `Industry` / `Concept` / `Region`（`signals.rs:464-473`）。但能力面只有两个布尔：

```rust
pub struct MarketRankingCapabilities {
    pub volume_ratio: bool,
    pub main_net_inflow: bool,
}
```

且闸门对其它一切**硬返回 `false`**（`signals.rs:502-508`）：

```rust
pub fn supports(self, kind: &MarketRankingKind) -> bool {
    match kind {
        MarketRankingKind::VolumeRatio => self.volume_ratio,
        MarketRankingKind::MainNetInflow => self.main_net_inflow,
        _ => false,
    }
}
```

⇒ **行业排行不是"没人实现"，是能力闸门在结构上无法放行。** 契约对外承诺了三个变体，能力层却没有任何字段能让它们为真。这是本仓"死契约变体"模式的又一例。

### 5.2 治理倒挂：唯一的行业通道未登记

TDX 板块族（`BoardDirectory` / `BoardConstituents` / `BoardMemberships`）走的是 `admitted(...)` **字面量**（`grpc_production.rs:2614-2618,2632-2636,2649-2653`），**无 `_ADMITTED` 常量**（全仓无 `BOARD_*_ADMITTED`）。

`admissions.tsv` 里 `Tdx` **只有 1 行**（`:18`，`SECURITY_PROFILES_ADMITTED`）——**三个 board op 一个都没登记**。

`tools/compliance/check_admissions.py` 只强制"声明了常量 ⇒ 必须有台账行"，所以它看不到这三个 op。与第 10 个问题里 Cninfo / Gov / Sina / ThePaper 未登记是**同一类倒挂**：真正承载行业语义的通道没登记。

### 5.3 `data-sources-inventory.md` 的板块段同样失效

该文件 `:32-39` 的"三、板块/资金类"引用 `src/data_gateway/board_runtime.rs:13-15`（**该路径不存在**）与 BR-159 / BR-188（`business_rules.md` 只有 **BR-001…BR-068**）。`:37` 还声称板块成分用于"BR-159 产业链推演、选股板块绑定"。

与第 10 个问题 §4.3 是**同一份文档的同一类缺陷**：它描述的不是本仓。

### 5.4 `magic-market-analysis` 没有任何行业聚合逻辑

该 crate 里 `industry` 的两次出现都在测试构造里（`tests/analysis.rs:111`、`tests/breadth.rs:97`，均为 `industry: None`）。**行业维度的聚合/中性化在本仓不存在。**

## 6. 拟议修改

### 6.1 零运行时行为变更（可立即做，与 D6 无关）

- **补台账**：给 TDX 三个 board op 加 `_ADMITTED` 常量 + `admissions.tsv` 行（与第四、八、九份设计的台账格式绑定，见 §11.4）。
- **修 `MarketRankingKind` 的诚实性**（D5）：删掉 `Industry/Concept/Region` 或在契约文档里明确标注"当前无能力可放行"。
- **修 `data-sources-inventory.md` 的板块段**：去掉不存在的路径与 BR 编号。
- 在 `docs/integrations/README.md` 索引里给板块/行业族加一行边界说明。

### 6.2 需要先拍板才能动的

- **权威源与授权**（D1）——**本设计的先决条件**。没有它，D2/D3/D4 都无处落地。
- **契约形态**（D2）：扩现有 board 类型，还是新增行业分类类型 + op。
- **层级表达**（D3）。
- **PIT / 版次**（D4）——本条是本设计最贵、也最有价值的一条。

### 6.3 本设计明确不做

- 从 F10 `所属行业` 文本反推分类。
- 把 TDX `block_fg.dat` / 东财 BK 码当作申万使用。
- 给 `SecurityProfile.industry` 填一个无码、无层级、无版次的自由文本名充当"行业分类"。

**逐条有依据，见 §8。**

## 7. 待批准人拍板（按编号回答）

**D1 — 权威源与授权：申万 / 中证行业分类从哪来、能否机器可读地取得？**
- (a) 申万宏源官方（申万分类的权威源）
- (b) 中证指数公司官方（中证行业分类的权威源）
- (c) 先用 TDX / 东财自有标签做**近似**，但**在契约与文档里始终标注为自有分类，绝不称"申万"**
- (d) 不做，等有消费方（见 D6）

> **本设计的建议是 (d) 打底 + (c) 可选**：在没有消费方之前不建；若建，也绝不把自有分类包装成申万。理由是 §3.3 的既有裁定已否决过同类做法，且 (a)/(b) 的**再分发授权**完全未验证（§10.2）——这可能把它变成第八项那类 entitlement 问题，而不是"加个源"。

**D2 — 契约形态：行业分类怎么进契约？**
- (a) 扩现有类型：`BoardDefinition` 加 `taxonomy` / `level` / `parent` / `taxonomy_version`
- (b) 新增类型 `IndustryClassification` + 新 op（如 `IndustryClassifications`）
- (c) 复用 `BoardMemberships`，只在 `board_code` 里编码谱系（如 `sw2021:L1:801010`）
- (d) 只填 `SecurityProfile.industry`（**等价于 security-master D3，且只能填自有分类**）

> 建议：若做，选 (b)——行业分类与"板块"是不同的语义（板块是可交易的板块指数，分类是层级树），塞进 `BoardCategory` 会让 (ii) 的层级问题无解。(c) 是最便宜的中间态，但把层级编码进字符串会重蹈 §3.3 身份格式的覆辙。

**D3 — 层级怎么表达？**
- (a) 全三层（一级/二级/三级）显式字段
- (b) 只做一级
- (c) **用 parent 链表达层级**，不写死层数

> 建议 (c)。申万/中信的层数会随版次变，写死层数等于把版次问题（D4）提前焊死。

**D4 — 是否引入版次与生效日期（PIT）？**
- (a) 加 `taxonomy_version` + `effective_from` / `effective_to`，支持 as-of 查询
- (b) 只给当前快照 + 版次号，不支持 as-of
- (c) 不加（现状）

> 建议 (a)。这是 §4.iii，本设计唯一直接关系前视偏差的一条。**(b) 是折中**，但折中的代价是：下游仍然不能在回测里问"某日的行业归属"——那正是缺口本身。

**D5 — `MarketRankingKind::{Industry, Concept, Region}` 怎么处置？**
- (a) 从 enum 删除（诚实；但改契约哈希）
- (b) 保留，加 capability 布尔并真正实现行业排行
- (c) 保留现状，仅补文档说明"当前不可达"

> 建议 (a) 或 (c)。**不要选 (b)** —— 行业排行依赖 §4.i/ii/iii 全部落地，不是一个布尔能救的。若担心契约哈希，选 (c)。

**D6 —（只有你能答）下游做不做行业维度的分析？**
- (a) **做**行业中性化 / 行业轮动 / 行业因子 → §4 三条全要解决，D1–D4 全做
- (b) **只做个股**，行业只是展示用 → 只做 §6.1（台账 + D5 + 文档），零风险
- (c) **未来可能做** → 先只做 §6.1，并把本设计存档

> **D1–D5 的价值全取决于这一问。** 与第 10 个问题不同的是：这一问若答 (a)，缺口是**硬伤级**（前视偏差）；若答 (b)，缺口基本不存在。

## 8. 被否决的替代方案

| 方案 | 否决依据 |
|---|---|
| 把 TDX `block_fg.dat`（155 个）当作申万/中证行业用 | **§3.3**：`tdx-board-zs:218` 已裁定"TDX 并非权威源"，且 `block/mod.rs:18` 自陈"**非中证/申万**" |
| 从 F10 `所属行业` 文本反推分类 | **`tdx-public-security-profile.md:14-19`** 明文"labels and text are not interpreted as industry"；且文本无码、无层级、无版次 |
| 给 `SecurityProfile.industry` 填自由文本行业名充当分类 | 无码空间、无层级、无版次；等于把 §2.1 的 4 套自有身份混进同一个字段，无法区分来源 |
| 用东财 BK 码当通用行业码 | 东财自有码空间，非申万/中信；仅供 `BoardFlows` 与研报过滤使用（`reports.rs:477-483` 的匹配是**东财内部**一致性校验，不是权威性校验） |
| 用 `MarketRankingKind::Industry` 做行业排行 | **§5.1**：`supports()` 硬返回 `false`，结构不可达 |
| 加行业数据源但不带生效日期 | **§4.iii**：直接引入前视偏差，比不做更危险——错的数据比没有数据更难发现 |
| 用 `block_zs.dat` 的指数板块代替行业分类 | 指数成分 ≠ 行业分类（且 `board_provider.rs:30-32` 明确拒绝把 TDX 混合指数板块当归一化板块） |

**结论：以上七条在权威源（D1）确定之前都不可实施。**

## 9. 影响面与风险

| 变更 | 影响文件 | 风险 |
|---|---|---|
| §6.1 补台账 | `admissions.tsv`；`magic-tdx-rs` 加 3 个常量 | **低**。不动运行时 |
| D5(a) 删死变体 | `magic-market-core/src/signals.rs:464-473`；改 `contract_sha256` | 中。**契约变更**，需确认无外部消费者 |
| D2(b) 新增类型 + op | `magic-market-core` 新类型；`define_operations!`（`magic-market-service/src/lib.rs:37-103`）；proto（4 个 service 之一）；`grpc_production.rs`；新 provider | **高**。新增 op 的成本见 §11.5 |
| D4(a) 加版次/PIT | 新类型的字段 + as-of 查询路径 + 数据源侧的历史版次可得性 | **高**。**前提是权威源提供历史版次**——未验证（§10.1） |
| §6.1 修文档 | `docs/data-sources-inventory.md` | 无 |

**特别风险**：D4(a) 的成本主要不在本仓，而在**数据源是否提供历史版次与生效日期**。若权威源只给"当前一版"，则 PIT 只能靠本仓自行累积快照——那需要一个**存储**层，而本仓当前无任何存储依赖（`Cargo.toml` 无相关 crate），且第 10 个问题已确认 BR-031 类规则原则上禁止 caching/storage。**这一条必须在 Gate B 前验证。**

## 10. 未验证项（不写成结论）

1. **权威源是否提供历史版次与生效日期。** 申万/中信的现行分类可公开查阅，但**历史版次的机器可读可得性完全未验证**。这是 D4(a) 的前提。
2. **再分发授权。** 申万分类由申万宏源发布，中证行业分类由中证指数公司发布；**机器可读再分发是否需要授权、属不属于第八项的 entitlement-blocked 类，未验证。** 这是 D1 的前提。
3. **申万/中信的确切层级数与现行版次号。** 本仓零信息；设计里刻意不引用外部记忆中的具体数字（如"一级多少个"），需从权威源核实。
4. **东财 `hybk` 是否确为东财行业板块名。** 按字段名推断（`limit_pool.rs:209`），未取实样。
5. **`tdx:industry:{名称}` 是否已有外部客户端依赖。** `tdx-board-zs` 已在 D4 标为未确认（`:184`），本缺口继承该前提。
6. **`MarketRankingKind` 的 `Industry/Concept/Region` 是否有外部消费者。** 若有，D5(a) 的删除会破坏它们。
7. **TDX 协议 `industry: u16` 的码表含义。** `protocol/types.rs:149` 只有类型，无码表；`security-master-fields` D3(b) 已指出"需 code→name 权威对照表"。未验证该码表是否公开。
8. **是否有第三方（东财/同花顺）已在本仓之外暴露申万分类。** 未验证。

## 11. 价值评估与拆分建议（2026-10-15）

### 11.1 立即可做、零风险

- **§6.1 补 TDX 板块族台账**：与 D6 无关，纯治理修复。
- **§6.1 修 `data-sources-inventory.md` 板块段**：消除"本仓有板块/行业运行时"的错觉。
- **§6.1 在 `MarketRankingKind` 上标注不可达**（D5(c)）：零成本，避免以后的调用方以为能按行业排行。

### 11.2 本设计与第 10 个问题的关键差异

新闻那次，作者上一轮的四条建议**全部违规**，产出是一份"否决清单"。**这次相反：缺口是真的，规则没有禁止，缺的是权威源与版次。**

所以本设计的诚实结论是二分：

- **若 D6 = (b)（只做个股）**：§4 三条都不是缺口，只做 §6.1，成本近零，本设计可以存档。
- **若 D6 = (a)（做行业维度）**：§4.iii 是**硬伤级**——不是"数据不够多"，是"数据会给出错误的答案"。这是十一个设计里最接近"静默错误"的一项。

### 11.3 真正值得先做的不是代码

**D1 是一个事实调查，不是设计决策。** 在写任何代码之前需要回答：申万/中证分类的机器可读来源是否存在、是否可授权、是否含历史版次。

这三问的答案决定 §4 是"一周的对接工作"还是"一个 entitlement 阻塞项"。**在没有这三个答案之前，D2/D3/D4 都是空转。**

### 11.4 与其它设计的关系

- **与 `security-master-fields` 的 D3 是互补而非重叠**：D3 决定"填不填 `SecurityProfile.industry`"，本设计决定"槽位里放谁的体系"。**D3 落地不会缩小本缺口。**
- **与 `tdx-board-zs` 的 D4 相邻但正交**：D4 管身份格式，本设计管分类体系。
- **与 `admission-index-coverage` 共用台账格式**：§6.1 的补登应在那份设计的 D1/D2 定了台账键与 schema 之后做，否则重复劳动。
- **与第 4 项（PIT / 修订证据）同病根**：§4.iii 是 PIT 问题在行业维度上的一个实例。若第 4 项先做，本设计的 D4 可能可以直接复用其机制。
- 与 `trading-calendar`、`overseas-market-data` 无耦合。

### 11.5 若 D6 = (a)，成本量级（前置警告）

新增 op 的成本：`define_operations!`（`magic-market-service/src/lib.rs:37-103`）加一行 → `READ_OPERATIONS` 镜像（`magic-market-grpc-contracts/src/lib.rs:15-81`）→ proto 消息与 RPC → `grpc_production.rs` 注册与 `admitted(...)` → `admissions.tsv` 行 → provider 实现与确定性测试。**约 5–7 个文件**，与历次新增 op 相同。真正的成本不在文件数，而在 §10.1/10.2 的两个未验证前提。
