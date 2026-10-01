# 新闻与资讯能力缺口调查 — 2026-09-25

状态：基于本仓库当前代码、合同、准入登记和前两轮会话的有限实时样本；本报告不改变 Provider 准入或对外合同。

## 结论与口径

项目已有财经快讯、个股新闻、A 股公告、财务三表、研报、政策文件、美国 SEC 申报元数据和部分宏观发布观测。用户感到科技新闻和上市公司财务新闻不足，主要对应**缺少稳定的专题与事件索引**：现有接口把不同来源的原始文章、公告和结构化数值分别交付，没有一个已证实的统一接口能按“科技、财报、减持”等主题或事件类型查询跨来源结果。不能据此说原始公告或财务数据不存在。[新闻与公告模型](../../crates/magic-market-core/src/content.rs#L7-L34)、[服务 RPC 列表](../../crates/magic-market-grpc-contracts/proto/magic/market/v1/market.proto#L330-L379)、[GlobalNews 请求](../../crates/magic-market-composition/src/grpc_production.rs#L344-L357)、[公告请求](../../crates/magic-market-core/src/market_announcements.rs#L4-L24)

更紧急的是基础入口可用性：2026-09-25 对运行实例的默认 `GlobalNews` 请求（不指定 Provider）返回 `FailedPrecondition`，未取到新闻；原因是默认落到 Eastmoney，响应中出现未准入的 `insurance.eastmoney.com` 文章域名。该问题与“科技内容占比不足”是两个层次，应先恢复默认新闻入口，再评价具体品类覆盖。

下文将“有接口/已准入”“本次实时成功”“可直接消费的新闻事件”分开判断。`NewsItem` 的 `summary`、`content` 本来就是可选字段，`topics` 是来源标签容器；接口存在不等于正文、统一分类或全市场覆盖得到保证。[NewsItem 模型](../../crates/magic-market-core/src/content.rs#L7-L20)

## 现有能力盘点

| 能力 | 当前可证实的范围 | 边界和证据 |
| --- | --- | --- |
| `GlobalNews` | 已注册 WallstreetCn、Cailianpress/Cls、ThePaper、XinhuaFinance、Yicai、Yonhap Economy、Jin10、Eastmoney 等财经/快讯来源。证券时报只有诊断入口。 | 请求只有 `limit`，无主题、事件、日期或搜索词；调用选择单个 Provider。证券时报因来源署名问题未准入。[注册入口](../../crates/magic-market-composition/src/grpc_production.rs#L707-L724)、[来源范围](../../crates/magic-market-composition/src/grpc_production.rs#L808-L883)、[调用形状](../../crates/magic-market-composition/src/grpc_production.rs#L4405-L4422)、[准入登记](../integrations/admissions.tsv) |
| `InstrumentNews` | Sina 个股新闻，上海/深圳 A 股单标的，日期范围、条数和 `captured_through` 截止时间。 | 不是全市场所有证券，也无事件类型过滤；北京证券被 Provider 拒绝。[注册范围](../../crates/magic-market-composition/src/grpc_production.rs#L1075-L1082)、[请求](../../crates/magic-market-composition/src/grpc_production.rs#L350-L358)、[Provider 边界](../../crates/magic-sina-rs/src/news.rs#L219-L225) |
| `Announcements` / `MarketAnnouncements` | Cninfo 个股公告与全市场日期范围公告；个股公告另有上交所、深交所来源。返回标题、可选源分类、详情/PDF 链接及证据。 | 个股请求按证券/日期/条数，全市场请求按日期/条数；均无标准化“财报、股东减持、回购”等事件过滤。[注册](../../crates/magic-market-composition/src/grpc_production.rs#L727-L765)、[交易所来源](../../crates/magic-market-composition/src/grpc_production.rs#L2225-L2258)、[公告模型](../../crates/magic-market-core/src/content.rs#L23-L34)、[全市场请求](../../crates/magic-market-core/src/market_announcements.rs#L4-L24)、[个股请求](../../crates/magic-market-core/src/capital.rs#L644-L669) |
| `FinancialStatements` | Sina 三表，以及已准入的 HithinkFinance 财报来源。记录含报告期、可选财务期/公告日与数值行。 | 请求按证券和三表种类，没有财报发布事件、同比变化解读或与公告的统一绑定；HithinkFinance 还需运行时密钥。[注册](../../crates/magic-market-composition/src/grpc_production.rs#L653-L663)、[Hithink 注册](../../crates/magic-market-composition/src/grpc_production.rs#L1470-L1485)、[请求](../../crates/magic-market-composition/src/grpc_production.rs#L445-L450)、[模型](../../crates/magic-market-core/src/company.rs#L41-L55)、[密钥边界](../../crates/magic-market-composition/src/grpc_production.rs#L6282-L6299) |
| `CompanyFilings` | SEC EDGAR 申报**元数据**：公司、表格、申报日、报告期及官方文档 URL。 | 运行服务需要 `SEC_USER_AGENT`；文档正文、附件和 XBRL facts 不在此能力中，不能当作 A 股财报新闻。[模型](../../crates/magic-market-core/src/filings.rs#L291-L305)、[运行注册](../../crates/magic-market-composition/src/grpc_production.rs#L3056-L3085)、[官方集成范围](../integrations/sec-edgar.md#L3-L12) |
| `ResearchReports` / `ResearchDocuments` | 东财个股或行业研报元数据；另有按精确身份获取研报文档的独立操作。 | 研报不是媒体新闻，须保留机构、日期和来源身份，不宜混入新闻标题流后丢失类型。[研报模型](../../crates/magic-market-core/src/research.rs#L115-L139)、[请求](../../crates/magic-market-core/src/research.rs#L651-L675)、[注册](../../crates/magic-market-composition/src/grpc_production.rs#L2728-L2746) |
| `SemanticSearch` | iWencai 授权语义搜索的模型含 `Report`、`News`、`Announcement`、`General` 频道。 | 当前准入证据只覆盖精确的 `Report` 频道；实现会将请求频道映射并传给上游，尚未见仅限 `Report` 的生产校验。因此 `News`/`Announcement` 属于**代码可请求、准入证据未覆盖**，不能当成稳定的科技新闻或公告搜索能力。即便其他频道未来获准，此操作也是独立来源搜索，不是现有八路新闻的统一索引。[频道与请求模型](../../crates/magic-market-core/src/research.rs#L620-L639)、[准入范围](../integrations/iwencai-api.md#L25-L37)、[频道映射](../../crates/magic-iwencai-rs/src/lib.rs#L389-L412) |
| `PolicyDocuments` | 国务院政策库按关键词、日期、页码检索，返回官方政策文档。 | 是独立政策文档查询，并非已分类的政策新闻/产业影响事件。[模型与请求](../../crates/magic-market-core/src/policy.rs#L6-L38)、[注册](../../crates/magic-market-composition/src/grpc_production.rs#L784-L800) |
| `EconomicReleaseObservations` | 金十当前滚动窗口中已经出现的结构化宏观发布，至多 20 条，可选精确国家。另有 FRED 发布日期级日程。 | 观测不证明完整经济日历；`EconomicCalendar/Jin10` 准入仍受阻，不能从快讯倒推出未来完整日程。[观测合同](../../crates/magic-market-core/src/calendar.rs#L100-L164)、[注册](../../crates/magic-market-composition/src/grpc_production.rs#L1978-L2001)、[准入登记](../integrations/admissions.tsv) |

另有 `CorporateActions`、`HolderCounts`、`LockupEvents`、`DividendPlans` 等独立数据操作。它们有助于公司事件分析，但不能等同于统一的上市公司新闻流。[RPC 列表](../../crates/magic-market-grpc-contracts/proto/magic/market/v1/market.proto#L342-L357)

## 2026-09-25 运行样本

本次又对运行实例的 `SystemService/GetCapabilities` 做了只读核对：能力目录列出 `GlobalNews` 的上述已准入来源、`Announcements` 的 Cninfo/SSE/SZSE、`MarketAnnouncements` 的 Cninfo、`FinancialStatements` 的 Sina/HithinkFinance、`InstrumentNews` 的 Sina，以及研报、政策、宏观观测等入口；`EconomicCalendar/Jin10` 与 `GlobalNews/SecuritiesTimes` 标为未准入诊断能力。该目录确认**服务宣告的能力状态**，不证明某个请求当下成功或新闻内容完整，因此还需结合下述实际请求结果。[能力接口](../../crates/magic-market-grpc-contracts/proto/magic/market/v1/market.proto#L311-L313)、[能力目录实现](../../crates/magic-market-service/src/lib.rs#L359-L376)

前两轮会话对当时运行实例做了有限抽样；原始响应未作为本报告附件保存，因此下表是**会话实测摘要**，不是可复现的全量覆盖率统计。`GlobalNews` 八个 Provider 选择器中六个成功，共返回 100 条：WallstreetCn 20、Jin10 15、Cailianpress 20、ThePaper 12、XinhuaFinance 13、Yicai 20。对 AI、人工智能、Gemini、OpenAI、Anthropic、云、数据中心、网络安全、机器人、具身智能等词的粗略命中为 16/100；这里可能包含跨来源重复和综述文章，不能推断“全市场科技新闻覆盖率为 16%”。Eastmoney 因文章域名 `money.eastmoney.com` 不在现有 allowlist 内整批失败；Yonhap 请求超时。代码仍将两者注册为已准入来源，说明**源码准入与当时可用性不同**。[来源注册](../../crates/magic-market-composition/src/grpc_production.rs#L808-L883)、[东财文章主机校验](../../crates/magic-eastmoney-rs/src/news.rs#L450-L466)

另有一个更直接的默认路径问题：生产注册顺序先调用 `register_eastmoney`，其中注册 `GlobalNews/Eastmoney`，之后才注册 `GlobalNews/WallstreetCn`；注册表按顺序追加，未指定 Provider 时选首个已准入且运行可用的注册项，且 `GlobalNews` 无失败后换源逻辑。因此按当前代码默认选择的是 Eastmoney，而非源码注释所称的 WallstreetCn。**本次单独实时复测默认 `GlobalNews`（`limit=3`、不指定 Provider）返回 `FailedPrecondition`，报错东财文章域名 `insurance.eastmoney.com` 未准入；没有返回新闻记录。**这印证默认路径当前受东财 URL 校验影响，但一次调用不证明长期故障率。[调用顺序](../../crates/magic-market-composition/src/grpc_production.rs#L702-L724)、[东财新闻注册](../../crates/magic-market-composition/src/grpc_production.rs#L2935-L2954)、[注册与选择规则](../../crates/magic-market-service/src/lib.rs#L348-L400)

同一轮对 Cninfo `Announcements` 的 600519、002131 指定窗口抽样，获得《2025 年年度报告》《2025 年半年度报告》、业绩预告修正和“回购股份减持”等公告标题及 PDF 链接。这证明至少在该窗口里**原始披露入口有数据**；“回购股份减持”不能解释为大股东减持，更不能从标题推断减持数量、比例或财报业绩变化。[公告输出模型](../../crates/magic-market-core/src/content.rs#L23-L34)、[Cninfo 字段映射](../../crates/magic-cninfo-rs/src/lib.rs#L712-L727)

## 可确认的缺口与优先级

| 优先级 | 缺口 | 证据与影响 | 建议验收边界 |
| --- | --- | --- | --- |
| P0 | 上市公司**公告事件索引**缺位 | 公告只带来源分类/标题/PDF，按标的或日期查询；全市场单次请求上限 300 条，三表另行返回。当前对外合同没有可验证的“年报、半年报、业绩预告、股东减持、回购、分红”统一事件对象；现有 `CorporateActionCategory` 有回购却没有股东减持类别。用户难以直接订阅或筛选这些事件。[公告模型](../../crates/magic-market-core/src/content.rs#L23-L34)、[公告请求](../../crates/magic-market-core/src/market_announcements.rs#L4-L24)、[三表模型](../../crates/magic-market-core/src/company.rs#L41-L55)、[公司行动分类](../../crates/magic-market-core/src/lifecycle.rs#L9-L24) | 先用官方公告的 ID/PDF/证券/日期建立可审计的事件类型；区分股东减持、回购股份减持等主体与动作。数据字段只在来源能证明时提取，附原文证据，不从标题补造数值。 |
| P0 | 科技专题的**稳定入口**缺位 | 当前已注册 `GlobalNews` 来源定位于财经/快讯，ThePaper 固定财经频道、Yonhap 正式入口仅 Economy；`GlobalNews` 无 topic 参数。可偶然命中科技内容，却不能保证“科技”可检索或覆盖。[Provider 注册](../../crates/magic-market-composition/src/grpc_production.rs#L808-L883)、[ThePaper 来源](../integrations/thepaper-web.md#L1-L12)、[Yonhap 准入](../integrations/yonhap-rss.md#L12-L20)、[请求](../../crates/magic-market-composition/src/grpc_production.rs#L344-L348) | 定义科技主题标签和质量样本，再评估有明确许可、完整时间/来源证据的垂直来源；按现有 Gate A–D 准入后提供主题查询。 |
| P0 | 已准入新闻源的**运行可用性与默认路由** | 八个显式选择器中两个失败，且本次默认 `GlobalNews` 单独复测也失败：Eastmoney 是首个注册源，文章域名 `insurance.eastmoney.com` 未准入时整批拒绝且不自动换源。一次调用不等于长期故障率；韩联社显式调用另有超时。[东财约束](../../crates/magic-eastmoney-rs/src/news.rs#L450-L466)、[Yonhap 传输边界](../integrations/yonhap-rss.md#L41-L64)、[注册与选择规则](../../crates/magic-market-service/src/lib.rs#L348-L400) | 先查明默认选源与源码注释不一致的原因，复核上游域名变化和超时成因；任何 HTTP/TLS/端点范围变更遵守现有设计、注册表和准入门槛。记录每源成功率、拒绝原因和最近成功时间。 |
| P1 | 跨来源**统一查询、去重与主题规范**不足 | 默认 `GlobalNews` 选一个已准入 Provider，不会自动合并八源；来源 `topics` 语义不一，WallstreetCn 固定写“华尔街见闻”、东财固定“财经”、金十取源标签，不能直接当统一“科技/财报”分类。[注册顺序](../../crates/magic-market-composition/src/grpc_production.rs#L707-L724)、[服务选择](../../crates/magic-market-service/src/lib.rs#L372-L398)、[来源标签](../integrations/wallstreetcn-rss.md#L95-L100)、[东财标签](../../crates/magic-eastmoney-rs/src/news.rs#L84-L93)、[金十标签](../../crates/magic-jin10-rs/src/lib.rs#L787-L825) | 以原始 Provider/URL/发布时间为可追溯输入，建立跨源事件身份、去重与受控分类；保留每条来源证据及冲突。 |
| P1 | 公司新闻和政策/宏观信息的**关联视图**不足 | 个股新闻只有 Sina 的单股入口；公告、财务数值、研报、政策文件和宏观发布各自通过不同操作返回。跨品类时间线需调用方自己关联，当前 RPC 合同没有统一事件检索接口。[个股新闻](../../crates/magic-market-composition/src/grpc_production.rs#L1075-L1082)、[RPC 列表](../../crates/magic-market-grpc-contracts/proto/magic/market/v1/market.proto#L330-L379) | 先明确“新闻文章、法定披露、数值观测、研报”四类身份；用源 ID、时间和证券建立关联，不把不同类型伪装成同一篇新闻。 |
| P2 | 完整宏观日历及海外申报正文 | `EconomicReleaseObservations` 只覆盖当前滚动窗口；Jin10 完整日历未准入。SEC 只有元数据，正文和 XBRL facts 未实现。这些是特定资讯研究需求的边界，不应声称整个宏观或海外财报数据为空。[观测合同](../../crates/magic-market-core/src/calendar.rs#L154-L164)、[准入登记](../integrations/admissions.tsv)、[SEC 范围](../integrations/sec-edgar.md#L3-L12) | 分别按官方日程和授权正文建立独立合同；不得以发布日期或标题推断未获得的事实。 |

并购重组、IPO、监管问询/处罚等重大事项也应纳入后续事件分类范围：现有公告可能含相关原文，财经快讯也可能提及，但当前对外接口只呈现通用公告/新闻及少数独立数据操作，尚无这些事项的可审计、可筛选的统一事件合同。这里判断的是**结构化可用性缺口**，不是断言上游完全没有相关文章。[公告模型](../../crates/magic-market-core/src/content.rs#L23-L34)、[对外操作列表](../../crates/magic-market-grpc-contracts/proto/magic/market/v1/market.proto#L316-L379)

### 报告适用边界

本调查审查了当前仓库的核心模型、gRPC 暴露和 Provider 注册，并引用前两轮有限实时抽样；没有重新爬取全市场新闻、核对所有历史日期，也没有验证下游应用的真实消费链。仓库中的 [`data-sources-inventory.md`](../data-sources-inventory.md#L1-L6) 标称 2026-08-12，引用当前仓库并不存在的 `src/data_gateway/**` 路径，因此不能作为本仓库现时已实现聚合、推送或去重的证据。上述优先级是根据用户关心的科技与财务事件可用性提出的产品建议，不等于已批准的新 Provider、合同或端点范围。
