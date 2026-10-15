# Gate A 设计：TDX 板块族 `block_zs.dat` 暴露与 BR-026 对齐（2026-10-14）

状态：待批准（决策点见 §7，按编号拍板后进入 Gate B）
问题 op：`BoardDirectory`、`BoardConstituents`、`BoardMemberships`（无新增 op）

## 1. 问题

`block_zs.dat`（TDX 指数成分文件，110 个板块，含沪深300/上证50，`crates/magic-tdx-rs/src/block/mod.rs:17`）
在生产环境中**可达性为零**。它不是"被拒绝"，是**没有任何出口**。同时 BR-026 注册的行为
与部署实现存在三处脱节。

## 2. 三处已核实的脱节

### 2.1 规则与实现：两文件 vs 三文件

BR-026（`docs/business_rules.md:311-315`）声明：

> The Provider reads complete, version-stable `block_fg.dat`, `block_gn.dat`, and
> `block_zs.dat` snapshots as one atomic evidence batch. […] ordered within each
> instrument as Industry, Concept, then Unknown index membership

- 该行为的实现存在于 `crates/magic-tdx-rs/src/service/blocks.rs`：`BOARD_FILES`
  三元组含 `BLOCK_SZ → BoardCategory::Unknown`（`:14-21`），原子读取 + 三文件哈希
  入 batch_id（`board_memberships_with` `:144-226`）。
- 但生产 `BoardMemberships` handler 绑定的是 `TdxBoardProvider`
  （`crates/magic-market-composition/src/grpc_production.rs:2530-2531` →
  `register_tdx_boards_client` `:2607-2669`），其反向查询**只读两个文件**
  （`crates/magic-tdx-rs/src/board_provider.rs:168-180`），且 `BlockType::Index`
  被硬拒（`board_provider.rs:30-32`）。

即：BR-026 描述的能力有实现、无出口；生产执行的是两文件子集。

### 2.2 板块身份格式：两个实现互不兼容

| 实现 | 产出的 `board_code` | 出处 |
| --- | --- | --- |
| `TdxBoardProvider`（生产） | `tdx:industry:<名称>` / `tdx:concept:<名称>` | `board_provider.rs:219-220` |
| `BlockService`（BR-026 描述） | `tdx:block_fg.dat:<名称>` … | `service/blocks.rs:179` |

BR-026（`:320-321`）规定的身份是

> Board identity is the exact source filename plus exact source block name
> (`tdx:<filename>:<blockname>`)

即 BR-026 登记的是 `tdx:block_fg.dat:白酒`，**生产返回的是 `tdx:industry:白酒`**。
`parse_board_code`（`board_provider.rs:227-238`）只接受 `tdx:industry:` / `tdx:concept:`
两个前缀。**照 BR-026 构造 `board_code` 再调 `BoardConstituents` 的客户端会收到
`Unsupported`。** 这一条独立于 `block_zs` 是否暴露，本身就是需要修的不一致。

### 2.3 台账缺口

`docs/integrations/admissions.tsv` 全文无 `board` / `concept` 行（61 行台账，grep 零匹配）；
`crates/magic-tdx-rs/src/lib.rs:25` 只导出 `SECURITY_PROFILES_ADMITTED` 一个 `_ADMITTED`
常量，不存在任何 `BOARD_*_ADMITTED`。

**口径说明（避免过度结论）**：这是"账本不完整"，不是"违反 BR-009"。BR-009
（`docs/business_rules.md:154-159`）管的是能力广告需有 deterministic 合同测试 + 有界实网探针——
三文件原子读确实已有测试（`service/blocks.rs:403,495`）。注册表本身未规定每族必须有一行。
准确的表述是：README:47-48 声称"精确到数据族和 Provider 的当前状态以准入注册表为准"，
**该声明对板块族不成立**。

## 3. 生产可达性论证

| # | 事实 | 出处 |
| --- | --- | --- |
| 1 | `BoardMemberships` handler 调用 `TdxBoardProvider::board_memberships` | `grpc_production.rs:2657-2658` |
| 2 | `BlockService` 已构造 | `grpc_production.rs:2533` |
| 3 | 其唯一生产调用是 `concept_hits` | `grpc_production.rs:2543` |
| 4 | `concept_hits_with` 只读 `block_gn.dat` | `service/blocks.rs:238` |
| 5 | `BlockService::board_memberships` 生产调用点 = 0 | 仅 `examples/board_membership_probe.rs:31` |
| 6 | 三个 `Board*Router` 只在 router 测试中构造 | `magic-market-router/tests/discovery_routing.rs:265,303,369,402,637,656`；`tests/board_directory_fallback_regression.rs:68` |

`magic-market-composition` 从 router 仅导入 `quote_source, AcceptancePolicy, AttemptStatus,
FailureKind, QuoteRouter, RouterError, SourceError`（`grpc_production.rs:67-70`），全 crate
只构造一个 `QuoteRouter`（`:3341`），**无任何板块 router**。

结论：`block_zs.dat` 的唯一读取方无生产出口 → **该文件在生产中完全不可达**。

## 4. 为什么"直接加 `BoardCategory::Index`"不可行

`docs/superpowers/specs/2026-07-25-market-intelligence-discovery-design.md:69-70`
已给出否决理由：

> The first admitted slice excludes `block_zs.dat`. That file mixes index and regional
> semantics, so mapping it to one Core `BoardCategory` would mislabel source data.

`block_zs.dat` 同时含指数板块与地域板块，文件格式不提供区分。新增 `Index` 会把地域
板块标成指数——正是 `crates/magic-market-core/src/signals.rs:10-15` 的
`BoardCategory` 设计要避免的 mislabel。**加枚举解决不了这个问题。**

## 5. 关键发现：无需新增 RPC

三个 op 的请求体全部走 `CanonicalPayload` JSON，**不是 proto 枚举**：

- `BoardCategory` 是普通 serde 枚举（`signals.rs:10-15`），JSON 值 `"Unknown"` 已可解码；
  `BoardDirectoryRequest::new(category, limit)`（`core/src/discovery.rs:156-159`）**不校验
  category 取值**，composition 侧也不预校验（`grpc_production.rs:2626`）。
- `BoardConstituentRequest` 只带 `board_code: NonEmptyText`
  （`core/src/discovery.rs:187-193`）——一个字符串，provider 侧解析。

因此扩展现有三个 op 去覆盖 `block_zs`：
- **不改 proto**（`market.proto` 不动）
- **不改 `contract_sha256`**
- **不需要重新生成 client bundle，不需要 Mac 侧协同冻结新身份**

这与 `docs/integrations/2026-10-14-t0-evidence-gate-a-design.md` 同一条判断线：
能不动冻结合同就不动。**新增 op 方案的成本比扩展方案高一个数量级**（新增 op 需要
proto RPC + `Operation` 变体 + bundle 重建 + 对端四字段冻结）。

## 6. 拟议修改（推荐组合，待 §7 拍板）

假定 D1=改规则、D2=扩展现有 op、D3=沿用 Unknown、D4=统一到 `tdx:<filename>:<name>`：

1. **`TdxBoardProvider` 放行 `block_zs.dat`**
   - `block_type()`（`board_provider.rs:207-215`）与 `board_code()`（`:217-225`）增加
     `BoardCategory::Unknown → BlockType::Index` 映射，board_code 形如
     `tdx:block_zs.dat:<名称>`。
   - `parse_board_code()`（`:227-238`）增加 `tdx:block_zs.dat:` 前缀。
   - `NetworkBoardSource::records()`（`:30-32`）移除 Index 的硬拒。
   - **`Region` 仍保持 `Unsupported`**（`block_zs` 里的地域板块无法与指数板块区分，
     不冒充）。
2. **板块身份格式迁移（D4）**：将 `tdx:industry:<n>` / `tdx:concept:<n>` 统一为
   BR-026 规定的 `tdx:block_fg.dat:<n>` / `tdx:block_gn.dat:<n>`。这是对外可见的
   身份变更，需要 §7 明确拍板；若选择不改，则必须反过来改 BR-026 的 §2.2 表述。
3. **BR-026 文本对齐（D1）**：若选"改规则"方向，把 `:311-327` 改为描述
   `TdxBoardProvider` 的实际两文件行为 + `block_zs` 作为独立第三族；若选"改实现"
   方向，则把 `BoardMemberships`/`BoardDirectory`/`BoardConstituents` 三个 handler
   切到 `BlockService` 并为其补 `BoardConstituentProvider`（`BlockService` 目前只实现
   `BoardMembershipProvider` `:285-295` 与 `ConceptHits` `:297`）。
4. **台账补行**：`admissions.tsv` 增补板块族行（crate `magic-tdx-rs`，provider `Tdx`，
   常量 `BOARD_DIRECTORY_ADMITTED` / `BOARD_CONSTITUENTS_ADMITTED` /
   `BOARD_MEMBERSHIPS_ADMITTED`），使 README:47-48 的权威声明对该族成立。
5. **测试与证据**：确定性测试覆盖 zs 放行、Region 仍拒、身份格式、三文件哈希
   batch_id；有界实网探针读取一次 `block_zs.dat` 并证明沪深300 成分可解析。
6. **文档**：`docs/TDX_CAPABILITIES.md` 板块节补 `block_zs` 行为与"非指数权威源"声明。

## 7. 待批准人拍板（按编号回答）

拍板结果（2026-10-15）：**仅治理方向**——D1a（改规则文本匹配生产）+ D4b（保留生产身份 `tdx:industry:` / `tdx:concept:`，改 BR-026 表述）；D2、D3 暂缓（暴露价值取决于消费方，未启动）；§6.4 台账补行随 admission-index 批次（其 schema 定案后）落地。以下原文保留。

- **D1 — BR-026 对齐方向**：(a) 改规则文本，使其匹配两文件生产实现；或 (b) 改实现，
  把三个 handler 切到 `BlockService` 使其匹配三文件规则。影响对比见 §6.3。
- **D2 — `block_zs` 暴露形态**：(a) 扩展现有 `BoardDirectory`/`BoardConstituents`
  （无 proto 变更，见 §5）；或 (b) 新增独立 op。推荐 (a)。
- **D3 — category 取值**：(a) 沿用 `BoardCategory::Unknown`（诚实，零新枚举）；或
  (b) 新增 `BoardCategory::Index`（会把地域板块 mislabel，§4 已论证不可行）。
- **D4 — 板块身份格式**：(a) 迁移到 BR-026 的 `tdx:<filename>:<name>`；或 (b) 保留
  生产的 `tdx:industry:` 并改 BR-026 表述。**这是对外可见变更，必须明确选择**——
  当前两者不一致本身即为缺陷（§2.2）。

## 8. 被否决的替代方案

- **新增 `BoardCategory::Index` 直接放行**：会把 `block_zs.dat` 中的地域板块
  标成指数，违反 `signals.rs` 的 mislabel 约束与 2026-07-25 设计记录。否决。
- **新增独立 op**：需要 proto RPC + bundle 重建 + 对端四字段冻结，而 JSON 载荷下
  扩展现有 op 零合同成本。除非 D2 选 (b)，否则否决。
- **复活 router 板块链**：三个 `Board*Router` 是 test-only 死代码
  （`discovery.rs:11-12`、`adapters.rs:45`）；生产只构造 `QuoteRouter`。复活它
  不增加任何出口，反而引入新的未准入路径。否决。
- **接中证/交易所官方成分文件**：这是正确的长期解法（带权重、生效日期、调样公告，
  且 TDX 本非指数权威源），但属独立数据源准入，不在本设计范围。**记录为后续项。**

## 9. 影响面与风险

- **爆炸半径**：若 D2=(a)，仅 `board_provider.rs` 一个 provider 的三处映射函数 +
  台账 + 文档；proto 与 `contract_sha256` 零变化。
- **对外可见变化**：`BoardDirectory(category="Unknown")` 从 `Unsupported` 变为返回
  110 个板块；`BoardConstituents` 接受 `tdx:block_zs.dat:` 前缀。若 D4=(a)，
  `board_code` 格式变更**会破坏既有客户端**——需确认当前是否有外部消费者依赖
  `tdx:industry:` 格式。
- **语义风险**：`Unknown` category 下的板块既含指数也含地域，客户端**不得**将其
  当作指数成分权威数据。必须写入文档与 op 描述。
- **残留限制**：无权重、无调样日期、无历史成分、`source_at` 仍缺省
  （TDX 对这些文件不提供源时间）。`block/query.rs:23-49` 的 18 条手写
  `index_code_to_name` 映射表**不引入**生产路径——文件按板块名组织，按指数代码
  索引是猜测，且表内已有歧义（`"000300"`/`"399300"` 同指沪深300；`"000001"`
  与平安银行代码冲突）。
- **Gate C 提醒**：本仓无 PowerShell 7，`tools/docs/check_links.ps1` 不可运行，
  文档链接检查需人工完成。

## 10. 未验证项（不写成结论）

- 未运行实网探针。`block_zs.dat` 能否被 TDX 服务器实际取到并解析出沪深300 成分，
  静态追踪无法证明。验证方式：`cargo run -p magic-tdx-rs --example board_membership_probe`
  （访问真实 TDX 服务器）。
- 未确认是否已有外部客户端依赖 `tdx:industry:` 身份格式（D4 的风险前提）。

## 11. 价值评估与拆分建议（2026-10-14 补充）

本设计把两类改动捆在一起，价值差异显著，批准时应拆开。

### 11.1 治理修复（确定收益，低前提）

D1 + D4 + §6.4 台账补行。修的是 §2 的三处脱节：规则描述的行为无出口、板块身份
格式与注册规则不兼容、权威台账缺行。这三条**即使永远不暴露 `block_zs.dat` 也照样
成立**，且改动几乎不涉及运行时逻辑（文档 + 身份生成 + 常量 + 台账）。

其中 §2.2 是活陷阱：照 BR-026 构造 `board_code` 的客户端会收到 `Unsupported`。

### 11.2 数据能力（有前提）

D2/D3 决定是否暴露 `block_zs.dat`。收益是"从零到能取当前快照"，得到 110 个板块
的成分名单。边界明确：

- 够用：当前时点的分组筛选、实盘分层标签、流动性规模的粗略代理；
- 不够：无权重、无调样/生效日期、无历史成分、无 `source_at`、指数与地域板块
  在文件内无法区分（故 category 只能是 `Unknown`）。

**建议**：仅当存在一个"当前时点"的消费方时才做 D2/D3。若无消费方，只做 §11.1，
等官方指数源接入时一并做对。

### 11.3 负价值边界（必须在文档与 op 描述中写明）

无生效日期的当前成分快照**不得**用于历史区间回测——会同时引入前视偏差与幸存者
偏差，且失败是静默的。本仓 `crates/magic-hithink-rs/src/historical_coverage.rs:44-49`
已自述 `pit_guarantee: false`。

### 11.4 明确不做的长期路线

把 `block_zs.dat` 当作指数数据源。TDX 并非指数成分权威源。正确解法是中证指数
公司/交易所官方成分文件（带权重、生效日期、调样公告），属独立数据源准入。

## 12. 拍板与实施记录（2026-10-15）

文件性质：设计当时（2026-10-14）基于静态代码路径核实；本记录为拍板后的实施说明。

已实施（docs-only，本批次）：

1. **BR-026 全文重写**（`docs/business_rules.md`）为两文件生产实现（`block_fg.dat` + `block_gn.dat`），并按逐项核实修正了旧文本与生产不符的四处：(a) 三文件原子批 → 两文件读取；(b) `tdx:<filename>:` 身份 → 生产的 `tdx:industry:` / `tdx:concept:`（对应 D4b）；(c) 「精确重复折叠为首次出现」→ 重复即报错（`board_provider.rs:156-162` + `tests/board_provider.rs:158-166`）；(d) 「北交所不支持」→ 已验证的北京四/八/920 前缀支持、非 920 九前缀拒绝（`tests/board_provider.rs:120-155`）。另记录：空交集为显式错误而非空批（`tests/board_provider.rs:192-205`）、批 ID 为 `tdx-block-files:<family>:<observed-time>`（无文件哈希）。`block_zs.dat` 以「非准入面、指数/地域不冒充」表述保留在规则中。

未实施（记录）：

2. **D2/D3（block_zs 暴露）**：暂缓；前置问题「消费方是谁」未答。若启动，按 §7 推荐走 D2a（扩展现有 op、零合同）+ D3a（Unknown）。
3. **台账补行（§6.4）**：随 admission-index 批次落地（该批次定台账 schema）。
4. **`BlockService` 三文件实现**：保持现状（仅服务 ConceptHits；其 `board_memberships` 无生产出口），不迁移、不复活 router 链（§8 已否决理由不变）。
