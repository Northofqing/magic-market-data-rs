# Gate A 设计：新闻数据覆盖边界（2026-10-15）

状态：待批准（决策点见 §7，按编号拍板后进入 Gate B）

问题 op：无。本设计**不新增 op、不改任何合同的字段、不改 proto**。事实核查发现"新闻数据少"的主体部分是**业务规则强制的边界**，不是缺陷；真正待决的是三处治理问题（§4）。

---

## 1. 问题

用户提出"新闻数据少"。这个说法有两种互相排斥的解释，核实结果是第三种：

| 假设 | 核实结果 |
|---|---|
| (a) 源不够 | **不成立。** `GlobalNews` 已准入 9 个注册 / 8 个不同源 |
| (b) 每条记录字段不够 | **成立，但主体是规则强制的。** 5 条 BR 明文禁止正文、描述与推断标的 |
| (c) 调用方无法知道哪个源给什么 | **成立，且无规则覆盖。** 这是唯一的真缺口 |

本设计的结论：**新闻层的"薄"是设计意图。** BR-012 / BR-020 / BR-030 / BR-031 / BR-042 逐条把每个源钉在"元数据"上，BR-066 又把查询窗口钉在每源 20 条并**明文声明不主张历史或全市场覆盖**。想加正文、加标的关联、加归档、加检索，都要先改这六条规则。

写这份设计的过程中，作者上一轮的口头分析提出的四条建议**全部违规**，已在 §8 逐条记录，以免以后重复提出。

## 2. 今天的实际覆盖（逐项核实）

### 2.1 `GlobalNews`：9 个注册 / 8 个不同源

| 源 | 注册位置 | 单次上限 | `summary` | `content` | `instruments` | `topics` |
|---|---|---|---|---|---|---|
| **WallstreetCn（默认）** | `grpc_production.rs:714-731` | 50 | `None` | `None` | 空 | 常量 `["华尔街见闻"]` |
| **Cls / Cailianpress** | `:822-837` | 50 | ✅ `brief` | ✅ `content‖brief` | ✅ 真解析 | ✅ `subjects`/`tags` |
| ThePaper | `:838-845` | 20 | `None` | `None` | 空 | — |
| XinhuaFinance | `:846-853` | **13** | `None` | `None` | 空 | — |
| Yicai | `:854-861` | 50 | `None` | `None` | 空 | — |
| Yonhap | `:883-890` | 50 | `None` | `None` | 空 | — |
| Jin10 | `:2106-2111` | 20 | `None` | ✅（必填） | 空 | — |
| Eastmoney | `:2872-2878` | 20（BR-020） | `None` | `None` | 空 | — |
| ~~SecuritiesTimes~~ | `:872-882` **blocked** | 30 | — | — | — | — |

- `Cls` 与 `Cailianpress` 是**同一个 client 注册两次**（`:824` / `:833` 都传 `cls.clone()`，都映射 `ProviderId::Cailianpress`），所以**不同源只有 8 个**，不是 9 个。
- **`summary` 非空只有 Cls 一个**（10 个新闻解析器中）。
- **`content` 非空只有两个：Cls 与 Jin10**，其余 6 个 `GlobalNews` 源恒为 `None`（逐源已核实：WallstreetCn `rss.rs:439`、ThePaper `lib.rs:448`、Xinhua `html.rs:75`、Yicai `html.rs:86`、Yonhap `lib.rs:764`、Eastmoney `news.rs:86`）。Jin10 的 `content` 是**必填**：`parse_item` 在 `content` 为空时直接报错（`magic-jin10-rs/src/lib.rs:750-751`），取值在 `:776`，`summary: None` 在 `:775`，且 title 为空时**回退为 content 的副本**（`:752`）。
- **`instruments` 非空只有 Cls 一个**，且有别于其它源：不是从文本推断，而是读源侧 `stock_list[].StockID`（`magic-cls-rs/src/lib.rs:429-465`），经 `parse_stock_identity`（`:467`）与 `classify_associated_asset` 构造成真 `InstrumentId`。唯一的例外是 `InstrumentNews`（Sina），它把**请求里的**那只标的回填进 `instruments`（`magic-sina-rs/src/news.rs:979`）——那不是源侧提供的关联，是回显。
- WallstreetCn 的 RSS **携带** `<description>` 与 `<content:encoded>`，但解析器不解包，且有测试把这一点锁死：`NEVER_EXPOSE_DESCRIPTION` / `NEVER_EXPOSE_BODY`（`magic-wallstreetcn-rs/src/rss.rs:688-689`），字段断言在 `:722-725`。

### 2.2 请求形状：`GlobalNews` 是全家最窄的

| op | 请求结构 | 可检索维度 |
|---|---|---|
| `GlobalNews` | `LimitRequest { limit }`（`grpc_production.rs:353-355`） | **仅条数** |
| `InstrumentNews` | `InstrumentNewsRequestV2 { instrument, start, end, limit, captured_through }`（`:357-365`） | 标的 + 区间 + 截止时刻 |
| `PolicyDocuments` | `PolicyRequest { query, start, end, page, page_size }`（`magic-market-core/src/policy.rs:8-14`） | **关键词** + 区间 + 分页 |
| `SemanticSearch` | `SemanticSearchRequest { query, channel, limit }`（`research.rs:709-713`） | 关键词 + 频道（**默认禁用**，见 §3.4） |

**同一个仓库、同一周：政策发布有关键词检索，新闻只有条数。** 这个不对称是真实的，但见 §3 —— 它是规则造成的，不是遗漏。

**但 op 之外，本仓已有一条有界的新闻发现层（上一轮分析漏掉的一层）：** `search_recent_news`（`magic-market-composition/src/content_discovery.rs:230-328`）对 `NEWS_SOURCES`（`:23-32`，**硬编码 8 个源名**）逐个发 `GlobalNews { limit: 20 }`，再用 `EntityQuery`（1–4 个别名组的合取）在 `title` / `summary` / `content` 三个字段上做**大小写无关的子串匹配**（`:69-81`）。

- 它是**单次请求、内存内、不落盘、不建索引**的形态——正好落在 BR-031 允许的上限（BR-031 禁的是 storage / caching / search indexing，不是这种即时匹配）。
- 它的返回里**已经有逐源结果**：`SourceOutcome::Inspected { provider, records, source_complete }`（`:290-294`），`scope` 也自称 `LatestPerSource { limit: 20 }`（`:236-238`）。
- **但它没有上 wire**：`search_recent_news` / `NewsCandidate` / `discover_disclosures` 的全部引用只落在 `magic-market-composition`（源码、测试、example），`magic-market-service` 与 `magic-market-grpc-server` **零引用**——gRPC 调用方拿不到这一层。

### 2.3 披露侧（不是"少"的部分）

`Announcements`（Cninfo + Sse + Szse）、`MarketAnnouncements`（Cninfo 原生日期区间，schema v2）、`InvestorQuestions`（Cninfo）、`PolicyDocuments`（StateCouncil）、`OfficialPublications` / `OfficialPublication`（9 部委，8 准入）—— 这些都有日期区间甚至分页。

**且政府发布真的返回全文**：`OfficialPublication.content: String`（`magic-official-news-rs/src/lib.rs:164`），每个源有 `body:` CSS 选择器（`profiles.rs:21-26`），测试断言 `article.content == "原文第二段"`。

所以"新闻少"只发生在 `GlobalNews` 这一条线上，披露与官方发布两条线是完整的。

## 3. 规则边界：为什么"少"

这一节是本设计的核心。五条规则逐条禁止了看起来像"缺口"的东西。

### 3.1 BR-031（WallstreetCN）—— 最严的一条

`business_rules.md:523-532`：

> "It may expose only title, decimal article ID, exact canonical URL, publication time, publisher, language, topic, and provenance. **RSS descriptions, article bodies, media, article-page fetching, undocumented APIs, authenticated content, storage, caching, search indexing, and inferred instruments are prohibited.**"

这一条**同时禁止**：正文、描述、取原文页、**存储**、**缓存**、**建检索索引**、推断标的。

`summary: None` / `content: None` / `instruments: 空` 是执行结果，`NEVER_EXPOSE_*` 测试是执行手段。

### 3.2 BR-042（Xinhua / Yicai / SecuritiesTimes）

`:561-565`：

> "…retain only first-party title, ID, link, publisher, publication-time and topic metadata. **Bodies, descriptions, images, login state, cookies and inferred instruments are prohibited.**"

### 3.3 BR-030（Yonhap）

`:508-521`：

> "…summary and content remain absent and **article pages are never fetched**."

### 3.4 BR-020（Eastmoney）与 BR-012（Jin10 / ThePaper）

BR-020 `:494-506`：域名只作元数据链接、**不抓取**，且"the public page does not provide structured security identity, so records keep an empty instrument list and **may not be presented as instrument news**"。

BR-012 `:409-420`：Jin10 限未锁定的公共 type-0/type-2；ThePaper 限财经频道 `25951`，带外部 `link` 的行一律丢弃；**"Neither source may infer structured security identity from text."**

### 3.5 BR-066 —— 三条硬约束

`:1258-1280`：

1. **默认源是强制的**："An unpinned `GlobalNews` production query prefers the explicitly configured `WallstreetCn` handler rather than registration order."
2. **新闻语义检索不准入**："iWencai `SemanticSearch` repository admission covers the exact `Report` channel only. `News`, `Announcement` and `General` return typed `Unsupported` before transport even when a runtime API key is present."
3. **窗口与免责是强制的**："A news query inspects **at most 20 records per admitted Provider**… **No result from these bounded windows claims complete historical or full-market coverage.**"

### 3.6 规则与实现的交叉结论

**规则集是自洽的，而且解释了全部"缺口"：**

| 观察到的"缺" | 规则依据 |
|---|---|
| 没有正文 | BR-031、BR-042、BR-030 |
| 没有摘要 | BR-030、BR-031、BR-042 |
| 标的关联为空 | BR-012、BR-020、BR-031、BR-042（均禁止**推断**） |
| 没有归档 / 缓存 / 索引 | BR-031（storage, caching, search indexing prohibited） |
| 没有新闻语义检索 | BR-066（只准入 Report 频道） |
| 默认源是最窄的那个 | BR-066（强制 WallstreetCn） |
| 没有日期区间 | BR-066（每源 20 条窗口，RSS 源本质上只有"最新 N"） |

**关键区分：规则禁的是"从文本推断标的"，不是"源侧提供的结构化标的"。** Cls 读 `stock_list` 是后一种，所以它不违规——它的富不是特权，是**源不一样**。BR-020 自己就写明了 Eastmoney 的公开页"不提供结构化证券身份"。

## 4. 三个真实缺口

### 4.1 能力面上，调用方分不清源

`ContentCapabilities`（`magic-market-core/src/content.rs:210-216`）只有 5 个布尔：`instrument_news` / `global_news` / `announcements` / `market_announcements` / `investor_questions`。**它区分不了 WallstreetCn（只有标题）和 Cls（标题+摘要+正文+标的）。**

且它**没有上 wire**：全部引用都在 provider crate 内部（构造、测试、example），`magic-market-composition` 与 `magic-market-grpc-server` 无引用。调用方从 `GetCapabilities` 拿到的是 `Capability { operation, provider, scope }`，其中 `scope` 是散文：

- WallstreetCn：`"bounded official RSS metadata"`
- Cls：`"bounded public CLS financial-news metadata"`

**两句话读不出字段差异。** 一个调用方想知道"哪个源能给我关联到股票的新闻"，只能去读 9 份 `docs/integrations/*.md`。

**补强证据：本仓已经有人感到这个痛，并做了局部补偿。** `search_recent_news` 返回逐源的 `SourceOutcome::Inspected { provider, records, source_complete }`（§2.2）——但那报告的是"这个源返回了几**条**"，不是"这个源的哪些**字段**非空"。而这恰恰是 `EntityQuery::matches` 需要的答案：它把 `summary` / `content` 一并当成匹配文本（`:70`），却无从得知 8 个源里有 6 个这两项恒为空——于是"匹配正文"这件事在 6/8 的源上退化为"匹配标题"，且**不报错、不降级、不声明**。

这是**唯一一个无规则覆盖、且确实是缺口**的问题。

### 4.2 Cls / Cailianpress：无规则、无常量、无台账行，却返回最多

| 项 | Cls 的现状 |
|---|---|
| 业务规则 | **`business_rules.md` 全文搜不到 `cls` / `Cls` / `CLS`（0 命中）** |
| `_ADMITTED` 常量 | 无 |
| `admissions.tsv` 行 | 无 |
| 已上线的返回字段 | `summary` + `content` + `instruments` + `topics` |

对照：8 个受规则约束的源全部正常登记（`:20` WallstreetCn、`:22` Xinhua、`:23` Yicai、`:24` Yonhap、`:10-11` Jin10、`:19` Stcn），而**唯一没有规则的那个，是返回字段最多的那个**。

这不必然是错的——Cls 的字段可能完全合法（源侧结构化，非推断）。但它**没有被任何规则或台账确认过**。按 Gate B（"typed errors/strict completeness/provenance"），当前状态是"实现先行、规则缺席"。

### 4.3 `docs/data-sources-inventory.md` 描述的是另一套系统

该文件的"二、新闻类"（`:22-30`）记录了一条**本仓不存在**的管线：

| 文档声称 | 本仓实际 |
|---|---|
| "全局新闻聚合 (4 Feed)… BR-166 轮询, 默认 120s (`NEWS_POLL_INTERVAL`)" | 无轮询、无 `NEWS_POLL_INTERVAL` |
| "盘面新闻事件：4 feed → market_event pipeline (**simhash 去重**)"（`src/news/aggregator/feed.rs`） | `MarketEvent` 只有 `MarketAnomaly` + `SourceStatus`（`magic-market-core/src/market_event.rs:880-883`），**无新闻变体、无去重** |
| "新闻→市场事件→**产业链映射→选股**" | 本仓不做选股 |
| `src/data_gateway/global_news.rs`、`src/search_service/service.rs:1369` | **`src/` 目录不存在** |
| 引 BR-092 / BR-153 / BR-159-166 / BR-174 / BR-218 / BR-231 等 | `business_rules.md` 只有 **BR-001…BR-068** |

已核实：`Glob src/**` 无文件；`business_rules.md` 无 BR-166 / BR-174 等标题。文档 `:4` 自称"数据获取统一入口在 `src/data_gateway/**` (CLAUDE.md 架构规则)"。

**这很可能就是"新闻数据少"这个印象的直接来源**：拿着描述"4 feed 聚合 + 去重 + 事件化 + 选股"的文档，看本仓的"每源最新 N 条"，自然觉得少。二者不是同一层。

**该文档描述的那套系统是不是下游（Mac 侧）应用，本设计未验证（见 §11）。** 但无论它是谁，它躺在本仓的 `docs/` 下且不带声明，就会被当成本仓的地图——这与第八个问题里 `src/data_gateway/**` 失效路径是同一类缺陷。

## 5. 附带发现

1. **Cninfo / Gov / Sina / ThePaper 未登记。** `Announcements`、`MarketAnnouncements`、`InvestorQuestions`（`:735-786`，Cninfo）、`PolicyDocuments`（`:787-804`，StateCouncil）、`InstrumentNews`（`:1083-1088`，Sina）、`GlobalNews`/ThePaper（`:838-845`）都是 `admitted(...)` 字面量，**无 `_ADMITTED` 常量、无 `admissions.tsv` 行**。披露侧最厚的家族反而没登记，最薄的 RSS 头条登记得最全——治理倒挂。
2. **`NewsProvider` trait 只有两个方法**（`content.rs:218-224`：`instrument_news` / `global_news`），9 个新闻 op 里其余 7 个不走这个 trait。
3. **`content: content‖brief`**（`magic-cls-rs/src/lib.rs:389`）意味着 Cls 的 `content` 可能只是 `brief` 的回显，不保证是独立正文。**Jin10 的 `content` 已核实**：必填、独立正文（非 brief 回显）、`summary` 恒为 `None`（`magic-jin10-rs/src/lib.rs:750-751,775-776`）。
4. **BR-054**（`:889-912`）要求 `GlobalNews` v2 每条的完整 `SourceEvidence` 且严格最新在前；`InstrumentNews` v2 额外要求调用方给出精确 `captured_through`。**截止时刻机制只存在于 `InstrumentNews`，`GlobalNews` 没有。**
5. **`NEWS_SOURCES` 是硬编码的第二个枚举，且不与注册表校验。** `content_discovery.rs:23-32` 手写 8 个源名，与 provider 注册表并行维护；`:244-251` 只用**实时 capabilities** 过滤（`operation == GlobalNews && repository_admitted && runtime_available`）。后果：**一个已准入但没写进这个数组的源会被静默跳过，不报错、不出现在 `page.sources` 里**。新增第 9 个 `GlobalNews` 源时必须手工记得改这里，编译器不会提醒。这是本仓反复出现的"两处枚举、互不校验"模式的又一例。
6. **`search_recent_news` 的 `instruments` 盲区。** 它只在 `title` / `summary` / `content` 上匹配（`:70`），**不看 `instruments`**。所以"找某只股票相关的新闻"在这个 API 上只能靠股票名出现在文本里，而 §2.1 已核实：`instruments` 非空只有 Cls，且 8 个源里 6 个的 `content` 也为空。这条发现与 D1、D6 直接相关。

## 6. 拟议修改

### 6.1 零运行时行为变更（可立即做）

- 修 `docs/data-sources-inventory.md`：按 §7 D5 处置。
- 在 `docs/integrations/README.md` 的索引里给新闻族加一行边界说明。

### 6.2 需要先拍板才能动的

- `ContentCapabilities` 扩到字段级并上 wire（D1）——动 `magic-market-core` 与能力响应。
- Cls 的规则与台账处置（D2）——**本设计唯一涉及已上线行为的决策**。
- `GlobalNews` 是否加 `captured_through`（D3）。
- 四个未登记源的补登（D4）。

### 6.3 本设计明确不做

- 解包正文、抓取原文页、推断标的、归档、缓存、建索引、开新闻语义检索。**逐条有规则依据，见 §8。**

## 7. 待批准人拍板（按编号回答）

**D1 — `ContentCapabilities` 是否扩到字段级并上 wire？**
- (a) 扩：加 `summary` / `body` / `instrument_linkage` 三个布尔，并纳入 `GetCapabilities` 响应
- (b) 不扩结构，只在每个源的能力 `scope` 串里写清字段差异
- (c) 维持现状，调用方读 `docs/integrations/*.md`

> 建议 (a)。这是本设计识别出的唯一真缺口，且不动任何数据合同——`ContentCapabilities` 是能力声明，不是记录。§4.1 的补强证据表明，唯一的现有替代品（`search_recent_news` 的逐源 `records` 计数）回答的是**条数**而不是**字段**，替代不了 (a)。

**D2 — Cls / Cailianpress 怎么处置？**（无规则、无常量、无台账行，返回字段最多、且注册两次）
- (a) 承认现状：把 `docs/integrations/cls-web.md` 已有的字段描述升格为一条 BR，补 `_ADMITTED` 常量与台账行
- (b) 收紧到元数据（与 BR-042 同形），补常量与台账行
- (c) 维持现状，仅在文档里显式标注"未登记"

> 建议 (a)。§3.6 已论证：规则禁的是**推断**标的，Cls 用的是**源侧结构化**字段，两者不同类，现状很可能是合法的。但需要一条规则把它写下来，否则下次审计仍会当成越权。

**D3 — `GlobalNews` 是否加日期区间 / 截止时刻？**
- (a) 不动（RSS 源本质只有"最新 N"，BR-066 已把窗口钉死在每源 20 条）
- (b) 加可选 `start` / `end`，仅在支持的源上生效，其余 typed `Unsupported`
- (c) 只加 `captured_through`，对齐 `InstrumentNews` v2 与 BR-054，不加区间

> 建议 (c)。区间对 RSS 源无意义，但**截止时刻是证据机制**（BR-054 的意图），对新闻同样成立：调用方需要能说"我要某一时刻之前的"。加 (c) 是 `GlobalNews` v2 → v3 + `NewsProvider` trait 签名变更，成本需评估。

**D4 — Cninfo / Gov / Sina / ThePaper 是否补登？**
- (a) 补 `_ADMITTED` 常量 + `admissions.tsv` 行（与第八、九份设计的台账格式绑定）
- (b) 只补文档，不补常量

**D5 — `docs/data-sources-inventory.md` 怎么处置？**
- (a) 删掉"新闻类"里属于另一套系统的行，改为本仓 op 清单
- (b) 文件顶部加显著声明，说明它描述的是哪套系统
- (c) 整份移出本仓

> 建议 (b) 打底、(a) 收尾。在 (b) 落地前，这份文档会持续制造"本仓有新闻聚合管线"的错觉。

**D6 —（只有你能答）下游要新闻做什么？**

**本仓自己已经给了一个部分答案**：`search_recent_news` 的存在说明"按实体别名在上界窗口里找新闻"是一个真实用例，而且已经实现（只是没上 wire）。但那是**本仓**的用法，不是下游（Mac 侧）的用法——仍需你确认。
- (a) 事件驱动选股 → `instruments` 只有 Cls 有，这是**硬约束**，且见 §8
- (b) 舆情 / 情绪因子 → 标题 + 时间可能够用
- (c) LLM RAG / 检索 → **要做成"可检索的语料库"就必须先改规则**（BR-031 的 storage / caching / search indexing，BR-066 的频道准入）。注意**即时匹配已是合规上限，且本仓已有**（`search_recent_news`，§2.2）——所以 (c) 若指"索引 + RAG"是规则问题，若指"当场在窗口里找"则已存在
- (d) 人工看盘 → 现状够用

> D1–D5 的价值全取决于这一问。

## 8. 被否决的替代方案

以下六条都曾被提出（部分由作者上一轮口头分析提出），逐条有规则依据，**记录下来以免重复提出**：

| 方案 | 否决依据 |
|---|---|
| 解包 RSS `<description>` / `<content:encoded>` | **BR-031** 明文禁止 RSS descriptions 与 article bodies；BR-042 禁止 bodies 与 descriptions；BR-030 禁止取原文页。测试 `NEVER_EXPOSE_*` 是执行手段 |
| 给新闻挂**推断**标的 | **BR-012 / BR-020 / BR-031 / BR-042** 四条均明文禁止 inferred instruments |
| 归档 / 缓存 / 建检索索引做 RAG | **BR-031** 明文禁止 storage、caching、search indexing。**合规上限本仓已经达到**：`search_recent_news` 正是"单次请求、内存内、不落盘、不建索引"的形态 |
| 把默认源从 WallstreetCn 换成 Cls | **BR-066** 明文规定未钉定的查询**优先 WallstreetCn**，不由注册顺序决定 |
| 开放新闻语义检索（`SemanticSearch` 的 `News` 频道） | **BR-066** 明文只准入 `Report` 频道；`News`/`Announcement`/`General` 一律在传输前返回 typed `Unsupported` |
| 把多源结果合并成一个 batch | **BR-066** 明文禁止 "wrapped into one fabricated atomic `DataBatch`"；候选只能来自已准入的有界窗口，且不得声称完整历史或全市场覆盖 |

**结论：以上六条在规则修改之前都不可实施。** 若 D6 选 (c)，先决条件是改 BR-031 与 BR-066，那是另一个量级的工作。

## 9. 影响面与风险

| 变更 | 影响文件 | 风险 |
|---|---|---|
| D1(a) 扩 `ContentCapabilities` | `magic-market-core/src/content.rs:210-216`；10+ provider crate 的 `content_capabilities()`；`GetCapabilities` 响应 | 低。纯增量；但需决定是否给每个源如实填新 flag（Cls=true，其余=false） |
| D2 补规则 | `docs/business_rules.md`；`docs/integrations/cls-web.md`；`admissions.tsv`；`magic-cls-rs` 加常量 | **中。若 (b) 收紧，是已上线行为的能力收缩**，需确认下游不依赖 Cls 的 `instruments` |
| D3(c) 加 `captured_through` | `GlobalNews` v2→v3；`NewsProvider` trait（`content.rs:224`）签名 | 中。trait 变更波及 9 个新闻 crate 的 `global_news(limit)` 实现 |
| D4 补登 | `admissions.tsv`；4 个 crate 加常量 | 低。不动运行时 |
| D5 修文档 | `docs/data-sources-inventory.md` | 无 |

## 10. 未验证项（不写成结论）

1. **`docs/data-sources-inventory.md` 描述的那套系统是谁。** 已核实其 `src/**` 路径与 BR-166/174 等编号在本仓不存在，但**没有验证**它是下游 Mac 应用、历史项目、还是别的。§4.3 的结论只到"不是本仓"。
2. **Jin10 的 `content` 是否有规则背书。**（实现本身已核实：必填、独立正文、`summary` 恒 `None`，见 §2.1——待验证的只是规则依据。）BR-012 只说"unlocked public type-0 flashes and type-2 articles"，未提及正文；BR-030/031/042 都显式禁正文，Jin10 没有对应条款。可能与 D2 同类（规则未覆盖）。
3. **Cls 的 `stock_list` 是否只含 A 股。** `parse_stock_identity`（`magic-cls-rs/src/lib.rs:467`）只读到 `sh`/`sz` 前缀分支，函数体未读完；若上游含港美股，行为未知。
4. **`content: content‖brief` 的回显比例。** 未取实样，不知道 Cls 的 `content` 有多少是真正文、多少是 brief 回显。
5. **`GlobalNews` 是否真被下游使用、默认源是否被覆盖，无数据。**（本仓侧已核实：`search_recent_news` 会**逐一点名全部 8 个源**并各取 20 条，但它不指定 provider，所以走不到"默认源"这条分支——默认源机制由谁触发仍未验证。）
6. **D1 的 wire 兼容性。** 扩 `ContentCapabilities` 是否影响 `contract_sha256`，未验证。若无影响，D1 是纯增量。

## 11. 价值评估与拆分建议（2026-10-15）

### 11.1 立即可做、零风险

- **D5(b)**：给 `data-sources-inventory.md` 加声明。这是**直接消除"新闻数据少"错觉**的最便宜动作，改动是文档。
- 在 `docs/integrations/README.md` 索引里给新闻族加一行边界说明。

### 11.2 本设计真正的产出是"否决清单"

六条被否决方案（§8）全部有规则依据。这意味着：

- **"新闻少"不是数据层缺陷，是合规选型的必然结果。** 想变厚，只有两条路：换更富的源（每个都要一条新 BR + 一次准入），或改规则（BR-031 / BR-066，量级远超本设计）。
- **不要再提"加正文 / 加归档 / 加检索"。** 记在 §8，避免重复劳动。

### 11.3 真正值得做的只有一件

**D1(a)。** 它是唯一一个"既有真实价值、又无规则障碍、又不动数据合同"的改动：让调用方能从 `GetCapabilities` 读出"哪个源能给我关联到股票的新闻"。

在现状下，这个问题只能靠人工读 9 份文档回答——而这正是本次分析从头到尾在做的事。

### 11.4 与其它设计的关系

- **D4 与第八、九份设计共用台账格式**：`admission-index-coverage` 的 D1/D2 定了台账的键与 schema，本设计的补登应在那之后做，否则重复劳动。
- **D1 与第十份之外的协议工作无耦合**：`ContentCapabilities` 是 provider 侧类型，不进 proto 记录。
- **`search_recent_news`（§2.2）是本仓唯一现成的新闻消费者，也是 D1 的第一个受益者**：它现在没法知道该不该把 `content` 计入匹配文本（§4.1）。若 D1 落地，它可以按源能力决定匹配字段，而不是对 8 个源一律拼 `title + summary + content`。
- 本设计与 `security-master-fields`、`trading-calendar`、`overseas-market-data` 均无耦合。
