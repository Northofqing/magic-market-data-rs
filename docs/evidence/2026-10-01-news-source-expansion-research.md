# 宏观、政策、地缘政治、行业及股权事件来源扩展调查

核查日期：2026-10-01。范围：中国与 A 股投资研究相关的第一方发布、公告和新闻服务。本文只记录来源建议与公开证据，不改变 Provider、HTTP 端点或权限准入，也不证明任何候选已经生产可用。没有使用用户密钥、激活账户、申请订阅或绕过反爬。

## 结论

扩源的第一优先级是补官方原文和可核验的公司披露，而不是继续堆叠相似的财经快讯。建议沿三条线推进：**公告事件深化；国内政策和数据新闻稿补充；海外货币政策、制裁与出口管制补充**。行业来源按关注板块逐批增加；需要跨国突发新闻、媒体独家与实时连续覆盖时，再评估正式授权的通讯社服务。这是针对用户关注类别的工程建议，不是覆盖率实测结论。

公开机器入口的证据最清楚的是美联储、欧洲央行、欧盟理事会的 RSS，EIA 的数据 API，以及 OFAC、联合国的制裁名单下载。FederalRegister.gov 有免密钥 API，但其网页/XML 并非正式法律版本，法律效力和准确原文应绑定 govinfo 官方文件。国内部委大多只核实到官网列表和原文；不能把网站搜索接口或前端请求推断为对外 API。[Fed RSS](https://www.federalreserve.gov/feeds/feeds.htm)、[ECB RSS](https://www.ecb.europa.eu/home/html/rss.en.html)、[欧盟理事会 RSS](https://www.consilium.europa.eu/en/about-site/rss/)、[EIA 开发者入口](https://www.eia.gov/developer/)、[OFAC SLS](https://ofac.treasury.gov/sanctions-list-service)、[联合国名单说明](https://main.un.org/securitycouncil/en/content/un-sc-consolidated-list)、[Federal Register API 与版本说明](https://www.federalregister.gov/developers/documentation/api/v1)

金十已有已准入的滚动窗口宏观发布结果，保留 previous/consensus/actual/revised 等数值；建议将这些已有结构化结果与官方发布原文关联，不是重新实现整套预期/实际值。它仍不同于完整未来经济日历，新 MCP 也尚未给日历恢复的证据。[金十接入文档](../integrations/jin10-web.md)、[BR-061](../business_rules.md)、[准入登记](../integrations/admissions.tsv)

## 当前代码核查与本轮运行证据

本轮按当前工作区核查，包含既有未提交的新闻候选发现改动。本调查只新增本文，不修改业务代码或已有准入状态。CodeGraph 已先用于定位，索引返回内容未能完整对应当前候选实现，随后以当前文件和运行结果核对；旧审计报告只作背景。

| 能力 | 当前事实 | 提升空间与实际影响 |
| --- | --- | --- |
| 综合财经新闻 | [`content_discovery.rs:20`](../../crates/magic-market-composition/src/content_discovery.rs)列八个来源，每源最近 20 条；`Cls` 与 `Cailianpress` 属同一财联社来源。默认 `GlobalNews` 已明确选 `WallstreetCn`，见 [`grpc_production.rs:724`](../../crates/magic-market-composition/src/grpc_production.rs)。 | 最新窗口只能用于候选发现。未命中不能解释成“该源历史上没有报道”。需历史/增量合同、来源覆盖与失败信息；不要重复列出已经修复的默认选源问题。 |
| 内容与实体 | [`NewsItem`](../../crates/magic-market-core/src/content.rs)已有正文、摘要、证券、主题、语言与证据字段；实际映射中财联社/金十有部分正文或标签，多数首页/RSS 来源以标题和链接为主。候选搜索在标题、摘要、正文中匹配别名组，见 [`content_discovery.rs:69`](../../crates/magic-market-composition/src/content_discovery.rs)。 | 现有 source topics 不是跨源统一行业/国家/事件分类。缺少稳定实体关联、翻译别名维护、事件聚类与跨源去重；全文获取必须另行验证授权和页面合同。保留原始标签与来源明示证券，新增推断字段不能伪装成源端发布。 |
| 公司公告发现 | [`DisclosureKind:102`](../../crates/magic-market-composition/src/content_discovery.rs)只有年报、半年报、股东减持、回购股份减持、业绩预告五类；[`classify_disclosure:400`](../../crates/magic-market-composition/src/content_discovery.rs)以标题规则判断。市场单日候选最多 300 条，个股范围最多 200 条。 | 增持、定增/配股、协议转让、控制权变化、质押/冻结、员工持股与激励、并购等未进入这套候选分类。即使减持已匹配，也未提取股东、数量、比例、价格、交易日期或实施阶段；公告原文/PDF 链接不等于已完成附件内容解析。 |
| 股权与公司行动 | [`HolderCount`](../../crates/magic-market-core/src/capital.rs)是股东人数，不是十大股东/控制链；[`CorporateActionCategory`](../../crates/magic-market-core/src/lifecycle.rs)已含 `AdditionalIssuance` 等类型。Hithink 生产 scope 明确只覆盖已实施现金/送股除权事件，见 [`grpc_production.rs:107`](../../crates/magic-market-composition/src/grpc_production.rs)。 | 不能说完全没有增发或股本变化类型，也不能把已有类型当成完整增发进程。需要上市股东快照、实控人披露及计划—审批/注册—发行—上市—终止的事件关联；披露时点与实际权益变动时点分开。 |
| 政策与宏观 | Gov 已有政策文档检索，覆盖 `gongwen`、`bumenfile`；[`PolicyDocument:100`](../../crates/magic-market-core/src/policy.rs)保留机关、文号、分类、发布日期和原链接。已有 Pbc/Nbs 等经济数值、金十窄窗口已发布观察值以及部分官方发布日程。 | 部分部委文件已被 Gov 收录，不是全部从零扩源。还需部委新闻稿/发布会、政策解读原文、草案/正式/废止状态、成文/生效日期和配套关系；用原文关联已有数值，不从媒体标题生成经济数据。 |
| 对外检索与事件推送 | 候选发现目前在 Rust composition 和 CLI example；[`GlobalNews` 请求](../../crates/magic-market-composition/src/grpc_production.rs)只有 limit。现有 gRPC 合同未提供统一按时间/实体/主题的新闻检索，[`MarketEvent`](../../crates/magic-market-core/src/market_event.rs)当前只有行情异动与来源状态。 | 如下游需要统一新闻搜索、订阅或事件更新，应先明确 bounded query/分页/游标与事件合同；当前行情推送不能直接当作新闻订阅。 |

### 公告覆盖文档与代码不一致

当前 [`magic-cninfo-rs/src/lib.rs:37`](../../crates/magic-cninfo-rs/src/lib.rs)为 `MAX_RECORDS = 300`，[`MarketAnnouncementRequest`](../../crates/magic-market-core/src/market_announcements.rs)也拒绝超过 300 条；[`market_announcements.rs:121`](../../crates/magic-cninfo-rs/src/market_announcements.rs)在取得请求数量后停止读页，最终截断到 limit。它验证已读取前缀的总数、分页和顺序一致性，**不证明已经读取该日期全部公告**。

但现有 [Cninfo 文档](../integrations/cninfo-web.md)写的是全市场最多 10,000 条、334 页以及 limit 前验证全部远程页。这是应修正的文档/证据偏差。不能据此文档把当前 300 条窗口宣称为全日完整覆盖；本次没有改变合同或擅自扩大请求上限。后续应在明确资源上限下设计历史回填/分片/可续读与覆盖结果，不能只把数字改大。

现有 Cninfo 市场映射确实处理 `BJS -> Exchange::Beijing`，见 [`market_announcements.rs:289`](../../crates/magic-cninfo-rs/src/market_announcements.rs)。因此北交所的建议是补覆盖验收和独立官方交叉证据，不是声称代码完全不识别北交所。

### 新鲜验证结果

- `cargo test -p magic-market-composition content_discovery::tests --lib --locked --offline`：退出码 0，4 passed、0 failed、47 filtered。验证候选查询约束、坏源失败保留、日期范围和两类减持区分；不能证明全部新闻源实时可用或历史完整。
- `cargo run -p magic-market-composition --example content_discovery --locked --offline -- news "宏观|政策|增持|减持|增发|制裁|关税|利率"`：本轮 2026-10-01 只读运行退出码 0，结果 `PartialSourceFailure`。WallstreetCn 20、Cailianpress 20、ThePaper 12、Yicai 20、Jin10 15、Yonhap 20 条被检查；XinhuaFinance、Eastmoney 本次失败，没有用其他源填补。这里是单次滚动窗口观察，**不是六个来源长期可靠或另两个永久不可用的统计结论**。CLI 隐去具体错误，本轮没有进一步确定两个失败的原因。
- 候选命中仅说明这些最新窗口包含相关词；各源更新速率不同，固定 20 条代表的时间长度不同，不能以命中数比较覆盖率。

## 现有来源与新增候选的界限

仓库既有调查和准入记录表明，当前基线包含财经快讯、Sina 个股新闻、Gov 政策文档、Cninfo/SSE/SZSE 公告、SEC 申报元数据，以及 Pbc、Nbs、Cfets、Fred、WorldBank 等数值数据。财联社、金十、华尔街见闻、新华社财经、第一财经、澎湃财经、东方财富与韩联社 Economy 已属于现有新闻来源。**下表把这些来源的频道、授权或文档能力扩展标成“现有扩展”，不把它们计为全新的新闻源。**现时功能和运行状态应以本轮主审的代码核查为准；2026-09-25 的默认路由故障和频道缺口不能未经复核就当成现状。[既有能力调查](2026-09-25-news-information-gap-audit.md)、[既有来源研究](2026-09-26-news-crawler-source-options.md)、[准入登记](../integrations/admissions.tsv)、[现行 BR-066](../business_rules.md)

下文“机器入口已证实”只表示发布方官网文档列出了 RSS/API/下载入口，**不表示本次已经取得权限、完整抓取载荷、验证所有分页或通过 Gate C**；“网页”表示核实到列表/原文，未确认公开机器合同。P0/P1/P2 是建议排序，不是准入状态。

## 候选来源清单（19 组）

| # / 优先级 | 来源与接入性质 | 类别和增量价值 | 本次证实的入口 | 准入风险与建议动作 |
| --- | --- | --- | --- | --- |
| 1 / P0 | **中国政府网：现有 Gov 扩展** | 国家政策正文、部门文件、政策解读；作为下属部委转载的去重和原文回链基线。 | [政策文件库](https://sousuo.www.gov.cn/zcwjk/)明确收录已公开的行政法规、规章、行政规范性文件，部门文件来自部门官网；[现有新目录](https://sousuo.www.gov.cn/zcwjk/policyDocumentLibrary)还列政策解读。网页已证实；没有确认面向外部的公开 RSS/API 文档。 | 不把政策库等同于全部时政或突发新闻。先检验现有 Gov 能否完整保留部门、文号、成文日、发布日期、实施日与解读关联；新增路径单独登记。 |
| 2 / P0 | **中国人民银行、国家统计局：现有数值源的新闻扩展** | 降准降息、货币政策报告、公开市场公告、统计新闻稿和答记者问。 | [央行官网](https://www.pbc.gov.cn/)有政策解读和公开市场业务；[统计局发布日程](https://www.stats.gov.cn/xxgk/sjfb/fbrcb/)与[2026 日程说明](https://www.stats.gov.cn/xw/tjxw/tzgg/202512/t20251224_1962137.html)可验证官方安排。新闻网页已证实；没有确认公开新闻 RSS/API。 | 统计局说明进度指标在数据发布后约 3 个工作日更新，新闻稿与数据发布库不是同一时效合同。现有 Nbs 数值接口不能充当即时新闻稿。日程、数值发布和政策声明分别保留身份。 |
| 3 / P0 | **发改委：新增专门新闻/政策候选** | 投资、重大项目、产业方向、价格调控和规划；补直接影响基建、能源与制造业的政策原文。 | [新闻发布列表](https://www.ndrc.gov.cn/xwdt/xwfb/)实际列出专题/月度发布会、成品油价格调整和项目协调信息。网页已证实；没有确认公开 RSS/API。 | 列表混有领导活动、仪式等，先选新闻发布与政策文件栏目，避免“官网全部文章=投资事件”。国务院政策库已有转载应按文号和原文 URL 去重。 |
| 4 / P0 | **财政部：新增专门新闻/政策候选** | 财政收支、预算、国债、税费、补贴与设备更新政策。 | [信息公开目录](https://www.mof.gov.cn/gkml/)提供通知公告、财政数据；[财政收支专题](https://www.mof.gov.cn/zhengwuxinxi/redianzhuanti/quanguocaizhengshouzhiqingkuang/)提供月份列表。网页已证实；没有确认公开新闻 RSS/API。 | 保留发布单位和数据累计期间，不能把“1—8 月”当作“8 月”单月值；税政原文与媒体影响解读分开。先核查目录抓取权限、更新和历史回填方式。 |
| 5 / P0 | **工信部：新增行业政策候选** | AI、电子信息、半导体、汽车、工业母机、通信、软件、工业产能；产业规划与产品目录。 | [政务公开政策文件页](https://www.miit.gov.cn/xbymdz/zwgk/index.html)有公文、机构、主题和日期筛选，并链接相关解读、相关新闻；[政策文件库](https://wap.miit.gov.cn/search/wjfb.html)列行业主题。网页已证实；没有确认公开 RSS/API。 | 规划、征求意见稿、正式通知、产品目录、政策解读分开分类；发布不等于正式实施。已被 Gov 收录的文件可以复用原文身份，不生成重复事件。 |
| 6 / P0 | **商务部、海关总署：新增经贸/贸易风险候选** | 贸易谈判、关税、进出口、反倾销、出口管制与外资；连接宏观和行业风险。 | [商务部新闻发布](https://www.mofcom.gov.cn/xwfb/)及[政务公开](https://www.mofcom.gov.cn/zwgk/)有发言人、发布会、政策、统计；[海关政务服务](https://online.customs.gov.cn/)链接署令公告、海关统计资料、政策解读和统计查询，[官方英文月度表](https://english.customs.gov.cn/statics/report/monthly.html)可作原始表入口。网页已证实；没有确认公开新闻 API/RSS。 | 海关企业业务数据交换接口不是新闻/全国贸易统计 API。贸易措施需地区、商品范围、税率/管制类型、起止和原始公告；不从新闻标题生成适用企业名单。 |
| 7 / P0 | **国家能源局：新增行业监管候选** | 电力、煤炭、油气、新能源、储能、绿证和充电设施；同时服务行业新闻和政策。 | [官网](https://www.nea.gov.cn/)有新闻发布以及通知、公告、项目核准、解读，列电力市场交易量、用电量等发布。网页已证实；没有确认公开 RSS/API。 | 政策/核准/统计分开，规划容量与已投产容量不能混用。与 Nbs 能源数值以及协会报告做证据关联，不互相补造源时间。 |
| 8 / P0 | **证监会与交易所监管发布：新增监管新闻栏目** | 再融资制度、并购重组、减持规则、监管措施、处罚、问询与审核进度。 | [证监会官网](https://www.csrc.gov.cn/)实际列政策解读、规章/公告、处罚、许可审核和监管措施；[深交所信息披露目录](https://www.szse.cn/disclosure/index/index.html)与[北交所公告页](https://www.bseinfo.net/disclosure/announcement.html)可定位相关栏目。网页已证实；没有确认普适公开新闻 RSS/API。 | 已接公司公告不等于已接交易所监管栏目。旧 `common_list.shtml` 本次呈现 2021 条目，首页链接是 `common_xq_list.shtml`，必须校验当前列表与历史归档，不能只因 HTTP 成功就视为“最新”。征求意见和生效规则、立案和处罚分开。 |
| 9 / P0 | **巨潮/上证/深证：现有公告深化；北交所：新增候选/覆盖验证** | 增持、减持、定增、配股、控制权、协议转让、股份质押/解押、回购、并购、发行上市等原始披露。 | [巨潮首页](https://www.cninfo.com.cn/new/index?lang=zh)有股权变动、增发等分类及北交所导航；[深证信数据服务](https://webapi.cninfo.com.cn/n)列公告资讯与 API 文档；[上证公告传输服务](https://www.sseinfo.com/services/other/announcement/)说明 WORD/PDF/XBRL/TXT；[公告文件说明书](https://www.sseinfo.com/services/data/szjr/ggwjfw/c/10010710/files/5f5989f49fb142f380fb027bb9997179.pdf)列按日元数据、公告分类和下载地址。北交所[公司公告列表](https://www.bseinfo.net/disclosure/announcement.html)默认最近一个月，可筛分类。 | 有正式服务说明不等于已有服务权限或再分发许可。优先利用现有 Cninfo/SSE/SZSE 原文，再评估正式传输服务和北交所覆盖；巨潮导航出现北交所不能证明当前 Provider 已完整取得北交所公告。需要历史页、市场范围、修订/撤回和原文一致性证据。 |
| 10 / P1 | **SEC：现有元数据扩展；HKEX：新增披露候选** | 美股/中概/H 股的重大持股变化、融资、并购、公司文件，给 A 股产业链和 A+H 主体提供交叉证据。 | [SEC 数据 API](https://www.sec.gov/search-filings/edgar-application-programming-interfaces)明确免认证提交历史和 XBRL；[HKEX FAQ](https://www.hkex.com.hk/Global/Exchange/FAQ/Getting-Started?sc_lang=en)说明权益披露及 IIS 信息服务，[IIS 标题传输规范](https://www.hkex.com.hk/-/media/HKEX-Market/Services/Market-Data-Services/Infrastructure/Issuer-Information-feed-Service-%28IIS%29/IISTransmissionSpecificationNewsHeadlinev1d9.pdf?la=en)是正式技术文档；[HKEX RSS](https://www.hkex.com.hk/services/rss-feeds?sc_lang=zh-hk)提供交易所新闻、规则等。 | HKEX 交易所 RSS 不等于全量公司公告流；[免费 News Alert](https://www.hkex.com.hk/Global/Exchange/FAQ/Getting-Started/News-Alert?sc_lang=en)限制 20 公司，公告每 30 分钟提醒、权益披露每交易日 17:30，不能作全市场实时 API。SEC 元数据不等于 Form 4/13D 等持股事项已经抽取；遵守其访问政策。 |
| 11 / P1 | **Fed、ECB：新增官方货币政策来源** | 利率决议、发布会、官员讲话、会议纪要/公告及政策沟通，补海外宏观原文。 | [Fed 官方 RSS 目录](https://www.federalreserve.gov/feeds/feeds.htm)列货币政策及讲话，明确供 reader/aggregator 订阅；[ECB RSS 目录](https://www.ecb.europa.eu/home/html/rss.en.html)列新闻稿、讲话、发布会全文等。**官方 RSS 发布已证实**，本次未完成 feed 载荷解析/稳定性验证。 | 先使用明确发布的 feed，保留 source URL、原文语言、发表时区、讲话人与文件类型。[ECB 使用条款](https://www.ecb.europa.eu/services/using-our-site/disclaimer/html/index.en.html)允许准确署名使用并要求标明修改，署名研究论文等有例外；不得把条件扩大到所有附件。 |
| 12 / P1 | **美国财政部 OFAC：新增制裁事件源** | 制裁新增/移除、许可和法规变更，支撑能源、航运、金融和供应链地缘风险。 | [Recent Actions](https://ofac.treasury.gov/recent-actions)按类别和日期列事件；[SLS](https://ofac.treasury.gov/sanctions-list-service)提供 SDN、非 SDN、定制数据和历史 delta 下载，并链接技术格式。**官方机器下载服务已证实**；本次未验证所有实际下载文件和 schema。 | 名单快照不是全量制裁新闻；需合并 Recent Actions 的动作证据、名单稳定 ID、别名与公告时间。模糊名称命中只是待核验关联，不能直接宣布上市公司被制裁。 |
| 13 / P1 | **美国 BIS + FederalRegister.gov：新增出口管制/法规源** | 实体清单、技术/设备出口限制、规则征求意见和生效变更，重点半导体、AI、军工和通信。 | [BIS News and Updates](https://www.bis.gov/news-updates)提供新闻和规则说明；[Federal Register API 文档](https://www.federalregister.gov/developers/documentation/api/v1)明确多种公开 API 且无需密钥。**官方 API 文档已证实**；BIS 新闻端未确认公开 RSS/API。 | 用机构、文号、正式文件关联 BIS 说明与法规。FederalRegister.gov 自述非正式法律版本，应回链 govinfo 官方 PDF；拟议规则、最终规则、实施日期必须区分。不能把 Entity List 与 OFAC SDN 当同一名单。 |
| 14 / P1 | **欧盟理事会：新增欧盟政策与制裁来源** | 欧洲制裁、对外关系、能源政策和经济金融会议决定。 | [新闻稿列表](https://www.consilium.europa.eu/en/press/press-releases/?keyword=sanctions)可按关键词/日期/主题筛选；[RSS 官方说明](https://www.consilium.europa.eu/en/about-site/rss/)明确新闻稿和会议的机器可读订阅。**官方 RSS 发布已证实**；本次未解析 feed 条目。 | [版权说明](https://www.consilium.europa.eu/en/about-site/copyright/)允许署名、保持原意并标明修改的再利用，第三方或特别标注材料有例外。新闻声明应关联正式法律文件与生效日期；不能把报道时间当制裁实施时间。 |
| 15 / P1 | **外交部 + 联合国安理会：新增国际风险原文** | 外交声明、记者会、制裁名单及委员会公告；提供可验证的国家/组织立场。 | [外交部发言人页面](https://www.mfa.gov.cn/fyrbt_673021/)有例行记者会和表态日期；[UN 安理会新闻发布](https://press.un.org/en/content/security-council/press-release)有委员会事件；[UN 综合名单说明](https://main.un.org/securitycouncil/en/content/un-sc-consolidated-list)明确 XML/HTML/PDF 下载和更新后新闻发布。**UN 官方 XML 下载发布已证实**；外交部未确认公开 RSS/API。 | 这是官方声明和记录，不保证战争/事故的分钟级全景。保留声明归属和交叉证据；UN 名单不同制裁机制必须分开。`/rss` 页本次呈现重复列表，不能据该路径名称声称已获得可靠新闻 RSS。 |
| 16 / P2 | **中汽协、SEMI：新增行业协会来源候选** | 汽车产销和新能源车行业；半导体设备、硅片、产业投资与供应链。 | [中汽协官网](https://www.caam.org.cn/)索引可见统计数据、行业政策与标准；[SEMI 发布档案](https://www.semi.org/en/news-media-press/semi-press-releases/press-archive?page=0)可按年份、SEMI/Member Press Release 筛选，并列设备/硅片发布。网页内容存在已证实；没有确认公开 RSS/API。 | 中汽协本次直接读取超时，搜索索引显示较旧统计，现时更新性未验证；SEMI 部分路径 403。先向发布方确认机器访问与报告许可。会员新闻稿属于企业自述，不能与协会统计同等处理；付费行业数据库不是新闻网页权限的附带内容。 |
| 17 / P1–P2 | **EIA 优先；IEA、OPEC 后续：新增国际能源来源** | 原油/天然气库存与供需、能源风险及 OPEC 决策背景，关联资源、化工、运输板块。 | [EIA RSS](https://www.eia.gov/tools/rssfeeds/)列新闻稿和 Today in Energy；[EIA API 文档](https://www.eia.gov/opendata/documentation.php)给 HTTPS v2、分页和免费注册密钥，[开发者页](https://www.eia.gov/developer/)说明 bulk 无需 key。[IEA OMR 说明](https://www.iea.org/events/oil-market-report-july-2026)明确订阅；[OPEC MOMR 页](https://www.opec.org/monthly-oil-market-report.html)列报告、Excel 附表和 2026 发布日程。 | **EIA RSS/API 文档已证实**，数据 API 不等于全部新闻全文 API；[API 条款](https://www.eia.gov/opendata/terms-of-service.php)允许查找/展示/分析并要求署名、禁止失实和绕限。IEA 摘要与付费全文分开；OPEC 有文档下载但未确认公开新闻 API/RSS和机器访问/再利用范围。 |
| 18 / P1–P2 | **财联社、金十：现有媒体升级；Reuters/LSEG：新增授权服务候选** | 专题检索、历史新闻、国际突发、行业与公司独家报道；官方来源以外的新闻发现与传播速度。 | [财联社官网](https://www.cls.cn/)证实媒体入口，但本次未找到可信的公开自助 API 文档。金十[官方 MCP 指南](https://mcp.jin10.com/app/doc.html)列 `list_flash/search_flash/list_news/search_news/get_news`，需登录激活 Token；[LSEG News API](https://developers.lseg.com/en/api-catalog/refinitiv-data-platform/news-API)明确 REST、流式/请求响应、元数据及身份/权限，[Reuters MCP](https://reutersagency.com/reuters-mcp)需内容订阅与 credentials。 | 金十正式机器文档已证实，**没有激活/测试 Token**；其[首页声明](https://www.jin10.com/)仍禁止未经授权商业使用，[旧免费服务终止说明](https://www.jin10.com/example/websiteiframe.html)仍在，新 MCP 文档没有证明经济日历恢复。Reuters/LSEG 是正式授权产品候选；网页可见、个人订阅、API 权限、历史缓存/再分发/AI 分析权限逐项确认。 |
| 19 / P2 | **企查查正式 API：新增可选工商结构资料** | 股东名称、持股比例、认缴、参股日期及企业年报股权变更，补非上市子公司与股东企业背景；属于结构资料，不是财经新闻。 | [工商登记股东 API](https://openapi.qcc.com/dataApi/731)明确 JSON、GET、分页及字段；[企业年报 API](https://openapi.qcc.com/dataApi/213)含股东/股权变更，默认最近三年。[企业服务说明](https://openapi.qcc.com/operation/generalize)列实控人与受益所有人产品，但本轮未确认对应细分接口合同。 | 要求企业实名、场景审核及开通。本次没有账户权限或 live 请求；工商登记股权不等于上市流通股持仓，披露/报送有时滞，也不自动证明最终受益所有人。优先用上市披露补足直接研究目标，有非上市控制链需求时再评估。 |

## 北交所与香港股权披露的补充核查

**北交所存在公告传输规范，但未确认普通外部客户端的开放读取权限。**[官方交易支持平台数据接口规范 V2.1](https://www.bse.cn/uploads/6/file/public/202209/20220902201701_kas8vw9r25.pdf)的信息公告文件中，GS 文件明确包含北交所上市公司信息公告摘要；[2026 年行情接口优化通知](https://www.bse.cn/class_b/200028461.html)还涉及信息公告文件调整。这是接入技术资料，不是免费公开 HTTP API 或已获接入权限的证据，历史规范也不能直接替代现时版本。公众可先查看[上市公司公告筛选页](https://www.bseinfo.net/disclosure/announcement.html)。仓库[Cninfo 接入文档](../integrations/cninfo-web.md)只给已登记 URL、映射、通用公告和既有沪深探针，没有明确的北交所公告准入/覆盖见证；官网导航存在“北交所”不能弥补这个证据缺口。这不等于断言现有 Cninfo 完全没有北交所记录；市场身份支持和实际结果须结合本轮主审代码证据判断。北交所官网是新增独立校验源候选，既有巨潮范围需要按明确窗口验收。

**香港披露易有快照与变更两个入口，完整公司公告可评估正式 IIS，权益披露的机器服务尚未确认。**[披露易](https://www1.hkexnews.hk/index_c.htm)区分上市公司文件、股权披露、股份回购等；[权益披露目录](https://www2.hkexnews.hk/shareholding-disclosures/disclosure-of-interests?sc_lang=zh-hk)进入现有和历史通知查询。[按上市法团查询](https://di.hkex.com.hk/di/NSSrchCorp.aspx?g_lang=zh-HK&lang=ZH&src=MAIN)可查指定日大股东完整/综合名单、董事名单和通知列表，[官方查询说明](https://di.hkex.com.hk/di/notes/NSAllSSList_C.htm)明确显示持有股份数、占投票权股份比例和最后申报日。这里只证明依法须披露的持股信息，不代表全部受益所有人或实时股权全景，也不能把 CCASS 托管参与者持股当成最终股东。

[IIS 现行产品页](https://www.hkex.com.hk/Services/Market-Data-Services/Infrastructure/Issuer-Information-feed-Service-%28IIS%29?sc_lang=en)在本次直接读取中列版本 4.8（更新 2026-09-29）和认证测试材料；[许可目录](https://www.hkex.com.hk/Services/Market-Data-Services/Real-Time-Data-Services/Data-Licensing/HKEX-IS/Market-Data-Vendor-Licence/Licence-Agreements_Guiding-Notes/Transmission-Specification?sc_lang=en)将 IIS 放在数据供应商许可下。因此已确认存在正式公司新闻/公告机器服务，但没有确认其当前许可是否覆盖独立 DI 查询、股东快照、全部历史与再分发；须向 HKEX-IS 核实，不把 IIS 泛化成“港股所有权益数据 API”。本报告没有使用仍处在计划/上线阶段的 Issuer Access Platform 作为现成公众数据接口。

## 面向用户类别的推进顺序

| 用户关注 | 优先组合 | 先验收什么 |
| --- | --- | --- |
| 增持、减持、增发、股权架构 | 既有 Cninfo/SSE/SZSE 原文深化 + 北交所覆盖验证；证监会/交易所审核与监管发布；必要时 HKEX/SEC | 主体、证券、事件阶段、数量/比例、定价或区间、公告日/实际日、完成/变更/取消、证据段落。权益结构要有股东快照和披露日期，再叠加事件；不能靠新闻条数推断完整结构。 |
| 国家政策 | 现有 Gov + 发改委/财政部/工信部/商务部/能源局原文 | 文号、发布机关、成文/发布/实施日、正式/征求意见/解读状态，以及原文与转载、配套措施之间的关系。 |
| 宏观新闻 | Pbc/Nbs 新闻稿 + MOF/GACC 发布；Fed/ECB RSS；需要时 EIA | 预定发布与实际发布分开，修订值留历史，单位和累计期间保真，新闻稿与数值序列关联但不伪造统一时间。 |
| 地缘政治 | 外交部/UN + OFAC/BIS/FederalRegister + 欧盟理事会；授权通讯社补突发报道 | 国家/机构/实体、措施类型、适用商品/地区、实施日期、更新/撤回；官方声明、媒体报道、企业回应分别标注。 |
| 行业消息 | 先 MIIT/NEA/MOFCOM，随后按关注行业加入 CAAM/SEMI、EIA/IEA/OPEC | 行业主题与证券关联标注证据和置信度；数据、监管、企业新闻稿、协会观点分开，避免将任何行业利好自动判成个股利好。 |

这里的字段和验收标准是后续设计建议，尚未批准合同变化。正式实施须按 [Gate A–D](../ENGINEERING_RULES.md)、[业务规则](../business_rules.md)、[准入登记](../integrations/admissions.tsv)和 [HTTP 传输登记](../integrations/http-transports.tsv)执行。

## 来源数量之外的必要提升

1. **先把发现变成可核验事件。**标题和来源标签用于候选召回；公司股权动作应读原文验证主体、动作和阶段。增持计划不等于完成，减持预披露不等于已卖出，回购股份处置不等于大股东减持，注册批复不等于增发完成。原始公告是公司披露证据，媒体解读另有身份。
2. **保留事件历程和修订。**同一政策或股权事项可能有计划、问询、回复、批准、实施、进展、结果、终止。建议以原公告 ID、文号、证券和主体关联事件；修订、取消和数值冲突留可追溯记录，不只保存最新标题。
3. **分别衡量速度与历史覆盖。**RSS 通常服务近期增量，不证明历史完整；官方档案和授权批量服务处理回填。每源统计窗口范围、覆盖日期、最后成功时间、延迟、重复率、缺失/拒绝原因；“来源失败”“没有匹配”“确证无数据”分别报告。
4. **实体和主题建立可维护关系。**公司全称/简称/曾用名、证券代码、股东名称/别名、子公司、行业、国家、商品/技术类别可关联，但推断关系和发布方明确关系要标注不同置信度。不能按人名或翻译名模糊匹配就自动判定制裁主体。
5. **授权边界落到字段。**机器访问、标题索引、正文/附件缓存、摘要/翻译、展示再分发、历史回填和 AI 分析各有范围。先保存被许可的字段及原链接。官方 RSS、公开网页、书面合同各自提供不同证据，不能互相代替。

## 建议实施边界与验收顺序

1. **第一批先补可核验公司事件与国内官方原文。**扩充公告候选类型，再设计原文抽取与阶段关联；先选增持、股东减持、定增、控制权变更四组有代表性的样本。国内源先补 Pbc/Nbs 新闻原文、财政部、发改委、工信部、商务部/海关、能源局和证监会当前发布栏目。具体来源可按关注行业缩减，避免同时接入大量低增量重复页。
2. **第二批补海外政策与地缘措施。**从已公开机器入口的 Fed/ECB/欧盟 RSS 开始，OFAC/UN 名单与动作公告关联，BIS 与法规正文关联。官方资料用于确认声明、名单和规则；冲突、突发事故及多方现场报道需要补授权通讯社，官方声明本身不代表完整事件经过。
3. **第三批按使用需求接入行业和付费服务。**能源重点 EIA，汽车 CAAM、半导体 SEMI 等先验证当期入口；需要历史搜索时评估金十正式 MCP 的实际查询范围，需要全球持续新闻时评估 Reuters/LSEG。股权背景涉及非上市控制链时再加入工商 API。均先取得样本和合同，不能根据“有搜索工具”承诺全量历史。

本仓库 [README](../../README.md)明确不承担数据库、数据湖、长期历史仓库或跨请求缓存，存储、批量调度由调用方负责。建议这里负责来源适配、受限查询/分页/游标合同、原始身份/证据和明确失败；**长期存储、跨源索引、去重聚类、控制关系历史和推断分析放在独立下游内容服务**。推断结果单独带算法版本和证据，不能写回成 Provider 发布字段。串行限速必须保留；如做异步并行调度须遵循 [blocking 集成规则](../integrations/async-blocking.md)，不能直接增加每源并发。

验收优先使用能暴露遗漏和误判的样本：同日公告超过 300 条；目标新闻已移出最近 20 条；同一政策官网原文与转载；减持计划后未实施/终止；定增已注册但未发行；同名股东；更正公告；来源暂时失败及恢复。应分别核实召回窗口、漏页、主体/阶段准确性、修订关联和失败状态。新增源的延迟、有效时间跨度、重复率、parse error 和最后成功时间必须在实际观察后再报告，不能预先承诺数字。

## 核查方法与未证实事项

- 使用网页检索和发布方官网读取；没有把转载文章、GitHub 爬虫或第三方 Python 库当作接口规范。API/RSS 的判断依据是第一方目录/技术说明，不依赖猜测路径。
- Fed 的货币政策 XML feed 链接由官方目录给出；本次网页工具报告 `text/xml` 不支持。ECB、Consilium feed 与 UN XML 的直接读取也未取得可解析载荷。因此报告只确认“官方发布此入口”，不声称实测数据条数、延迟、历史范围或稳定性。
- CAAM、CEC、BIS 部分页面超时/不可读取；SEMI 旧路径出现 403。没有更换身份或绕过访问限制。CEC 暂只保留为下一轮待验证协会，不列入已证实扩源建议；本次行业能源建议以 NEA/EIA 可核实入口为主。
- 财联社未核实公开 API 合同；金十已找到正式 MCP 文档但未取得权限，日历未获新证据；Reuters/LSEG/HKEX 的正式产品文档不等于本用户已获产品 entitlement 或发布权。
- 本文不做全量新闻抓取或覆盖率测量，不声称这 19 组能覆盖所有宏观、地缘、行业和公司事件。本轮最新窗口探针与局部测试见上文；旧证据文件中与当前代码不一致的限制需要单独修正，不在本研究中扩写为“已经实现”。
