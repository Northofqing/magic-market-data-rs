# 国内官方发布原文：八源准入

`magic-official-news-rs` 提供独立的 Rust 原文读取接口：
`OfficialNewsClient::latest(limit)` 和 `article(exact_url)`。它读取公开网站的
固定栏目，不使用账号、Cookie、验证码、浏览器会话或终端登录信息。
源目录有九个发布机构，当前正式准入八个；海关在正式方法中先于 I/O 拒绝。
`probe_latest` / `probe_article` 是显式诊断方法，不提升准入。

现已接入统一 Service 与 gRPC：`OfficialPublications` 返回当前列表，
`OfficialPublication` 返回同域原文；另有可显式启动的定时采集程序。
这两个方法采用独立的原生证据合同，见下文；既有 `GlobalNews` v2 合同保持不变。
独立的 `OfficialSource` 代表原站发布机构，不伪造已有 Provider 的身份。

## 已准入的精确范围

| Source | 当前固定栏目 | 列表条数（2026-10-01） | 正文时间精度 |
|---|---|---:|---|
| Nbs | [统计局最新发布](https://www.stats.gov.cn/sj/zxfb/) | 15 | 分钟 |
| Pbc | [央行沟通交流](https://www.pbc.gov.cn/goutongjiaoliu/113456/113469/index.html) | 15 | 日期 |
| Ndrc | [发改委新闻发布](https://www.ndrc.gov.cn/xwdt/xwfb/) | 25 | 秒 |
| Mof | [财政部综合司政策发布](https://zhs.mof.gov.cn/zhengcefabu/) | 10 | 日期 |
| Miit | [工信部部领导活动](https://www.miit.gov.cn/xwfb/bldhd/index.html) | 24 | 分钟 |
| Mofcom | [商务部日常新闻发布](https://www.mofcom.gov.cn/xwfb/rcxwfb/index.html) | 15 | 分钟 |
| Nea | [能源局新闻中心](https://www.nea.gov.cn/xwzx/index.htm)顶部局工作动态窗口 | 10（两组各 5） | 日期 |
| Csrc | [证监会要闻](https://www.csrc.gov.cn/csrc/c100028/common_xq_list.shtml)当前聚合页，含同域领导活动原文 | 18 | 日期 |

时间精度按每篇实际标签判定，表中是本次样本。列表日期与正文日期、标题均经过
样本交叉检查；客户端的两个方法返回各自证据，不把多次响应拼成原子快照。
已准入的是这些栏目及其同域固定路径的 HTML 正文，非整站所有栏目、所有政策文件。

静态列表分别使用 `.list-content > ul > li` 中的 `a.pc_1600`、
`font.newslist_style` 及其父节点日期、`.u-list > li`。
发改委的空 `.empty` 分隔项必须没有锚点和文本；含内容的分隔项会拒绝。
统计局正文使用 `.detail-text-content .TRS_UEDITOR`，央行使用 `#zoom`，
发改委使用 `.article_con > .TRS_Editor`，商务部使用
`div.art-con[ergodic=article]`。页面生成时间 `createDate`、`MakeTime` 不作发布时间。

商务部列表读取源页面的精确 HTTPS 首面 JSON 请求：

```text
GET https://www.mofcom.gov.cn/api-gateway/jpaas-publish-server/front/page/build/unit
parseType=bulidstatic
webId=8f43c7ad3afc411fb56f281724b73708
tplSetId=52551ea0e2c14bca8c84792f7aa37ead
pageType=column
tagId=分页列表
editType=null
pageId=95d89972d8aa4fcea511701cd0f212d9
```

`bulidstatic` 是来源实际拼写。请求值封闭，不接收调用方的任意参数。
JSON 必须是 `code="200"`、`success=true`、`data.html`；HTML 中必须有唯一
`div.pagination`，`rows` 对应全部条目、`pageNo=1`、`count>=rows`。
本文档不将这个前端请求称为有 SLA 或再分发授权的开放 API。
完整入口证据见[动态列表研究](../evidence/2026-10-01-domestic-official-listing-endpoints.md)。

追加准入范围经过[独立官方入口研究](../evidence/2026-10-01-official-news-admission-followup-research.md)
与[原生实测](../evidence/2026-10-01-official-news-admission-followup.md)：

- 财政部只读取 `zhs.mof.gov.cn/zhengcefabu/` 的 `ul.liBox > li`，正文限于同主机
  `/zhengcefabu/YYYYMM/tYYYYMMDD_ID.htm`。覆盖综合司发布的政策，非财政部全站新闻、
  税政或国债。标题 `h2.title_con`，正文 `.my_doccontent > .TRS_Editor`，日期取可见
  `.docreltime > span` 的 `发布日期：YYYY年MM月DD日`。
- 工信部读取官网 `api-gateway/jpaas-publish-server/front/page/build/unit` 的固定首面
  unit：`parseType=buildstatic`、`webId=8d828e408d90447786ddbe128d495e9e`、
  `tplSetId=209741b2109044b5b7695700b2bec37e`、`pageType=column`、`tagId=右侧内容`、
  `editType=null`、`pageId=d3e2bede1bc045e2875fc7161c01db7d`。JSON envelope 与商务部
  相同，但分页首面最多 24 行；唯一 `div.page-content` 父 ID 必须为 `右侧内容`，
  内部唯一 UL 的每个 LI 都校验，主锚点为 `a.fl`。正文限 `/xwfb/bldhd/art/YYYY/art_32hex.html`，
  标题 `h1#con_title`、正文 `div#con_con.ccontent`，发布时间取可见
  `.cinfo > span#con_time` 的 `发布时间：YYYY-MM-DD HH:MM`，不用冲突的 PubDate。
- 能源局只读取 `.xwzx-page01 .xwzx-yw-right .xwzx-yw-box` 中两个固定 UL。
  datasource ID 分别为 `91fb3999d1964141b668e4a4cef4ed98`、
  `64763711c745408cb6f3bc0895f37649`，顺序及各 5 行必须吻合，共用更多链接必须为
  `/news/jwzdt.htm`。整个窗口校验后再 limit。正文限同域 `/YYYYMMDD/32hex/c.html`；
  标题 `.article-title > .titles`、正文 `#detailContent`，发布时间取可见
  `.article-title > span.times` 的 `发布时间：YYYY-MM-DD`。不执行页面 HTTP 脚本。
- 证监会读取固定 `searchList/a1a078ee0bc54721ab6b148884c784a8`，固定参数为
  `_isAgg=true`、`_isJson=true`、`_pageSize=18`、`_template=index`、
  `_rangeTimeGte=`、`_channelName=`、`page=1`。响应必须声明数值 `page=1`、`rows=18`、
  相同 `channelId`、字符串 `relateSubChannels="true"`，结果数为 `min(total,18)`。
  列表保留 `publishedTimeStr`；原文只允许 `/csrc/c100028/c数字/content.shtml` 和
  `/csrc/c106311/c数字/content.shtml`。标题 `.main > .content > h2`、正文
  `div.detail-news`，日期取 `.main > .content > .info > p.fl` 的可见
  `日期：YYYY-MM-DD 来源：…`。PubDate 与生成时间含义存疑，不覆盖任何发布标签。

以上都是当前有界窗口，不因较窄范围准入而解除原财政新闻、工信部最新政策、
能源局政策库动态列表的旧阻碍。正文日期标签缺失、重复或变化均明确失败，没有元数据回退。

## 数据和传输合同

- 列表保留发布机构、栏目 URL、实际响应 URL、观察时间、原始响应字节的 SHA-256、
  应用 limit 前的完整验证条数，以及标题、原文 URL、日期、来源日期标签和精度。
- 正文保留原文标题、日期、实际日期/分钟/秒标签、精度、提取文本、观察时间及响应
  SHA-256。日期不补成午夜时间，不推断时区。文本不包含 script/style/noscript。
- `publication_label_origin` 明确标签位置：`ListingHtml` 为列表行（含 JSON 内的 HTML），
  `ListingApi` 为列表 API 字段，`ArticleMetadata` 为原文发布元数据，
  `VisibleArticleDate` 为可见原文日期/时间行。财政部、工信部、能源局、证监会使用最后一种。
  DOM 标签保留来源文字和标点，但折叠显示空白；元数据标签去掉两端空白。
  原始响应字节由 SHA-256 绑定，标签不声称保留 HTML 的字节排版。
- 一次只读当前一页。调用方 limit 为 1–20；源容器为 1–100 条，证监会页最多
  18 条、工信部最多 24 条，能源局恰好两组各 5 条。全部条目先验证，再截取 limit。
  重复 URL、非法日期、缺字段、空列表、
  非允许正文链接或模板变化均显式失败。来源日期超过观察 UTC 日期加一天会拒绝；
  一天余量只覆盖日历日期的时区边界，不声称获得了精确发布时间。
- 仅共享 `magic-market-transport`；固定 HTTPS 主机及路径，不跟重定向，15 秒超时，
  2 MiB 响应体、严格 HTML/JSON 媒体类型与 UTF-8 校验。原文 URL 禁止跨域、用户信息、
  参数、片段、编码路径及点路径。clone 共用串行门，请求起始至少间隔一秒。
- 不请求附件/PDF、引用页面、脚本、跨站原文；没有历史翻页、全文索引、事件推断、
  数据库或自动订阅。SDK 本身不调度；Composition 采集程序按轮记录查询结果。
  列表不是一整天、全市场或全部政策的完整覆盖。

## 未准入及旧范围边界

| Source | 明确阻碍 |
|---|---|
| Gacc（来源仍未准入） | 共享 Rust 请求在有效 HTTP 响应前失败；Windows 默认 TLS 报主机名和证书链错误，列表/正文未验证 |
| Mof 的原主站范围 | 财政新闻、主站政策发布、财政部令混有 HTTP 跨司局原文；新准入只覆盖综合司政策发布 |
| Miit 的原最新政策范围 | 原样本 24 条中 23 条为 HTTP；新准入只覆盖部领导活动 |
| Nea 的动态政策范围 | HTTPS 分类页、公开目录和完整局工作动态栏目为空动态列表，依赖 HTTP 脚本；只准入已验证静态十条窗口 |

海关正式调用先于网络返回 `Unadmitted`；统一服务也登记为禁用，
即使 `allow_unadmitted=true` 仍没有诊断 handler。八个正式来源的两个方法均进入
`GetCapabilities`，各自保留精确栏目范围。

## 统一接口与定时采集

Proto 追加两个操作：`OfficialPublications=64` 和 `OfficialPublication=65`。
请求、记录版本均为 1，`preferred_provider` 精确选择上表的 Source 名称；未指定时
默认 `Nbs`。原文 URL 必须属于所选来源，不跨源回退。

| RPC | 请求 schema | JSON 请求 | 单条返回的 schema |
|---|---|---|---|
| OfficialPublications | `magic.market.official_publications.request` | `{"limit":5}`（1..20） | `magic.market.official_publication_listing` |
| OfficialPublication | `magic.market.official_publication.request` | `{"url":"列表中的精确 HTTPS canonical_url"}` | `magic.market.official_publication` |

gRPC payload 的 `content_type` 必须为 `application/json; charset=utf-8`；
`data` 为对应 JSON 的字节（Proto JSON 表示使用 base64）。

每次返回一条完整原生证据 envelope，不拆散列表的 `source_rows`、`response_url`、
`response_sha256`、`observed_at` 与 `items`。原文保留内容和实际标签/精度/位置。
QueryResponse 的 `source_at` 为空；`complete=true` 仅证明本次有界响应全部通过验证。
batch_id 绑定来源、操作和完整返回 envelope；列表与原文各自独立。
同一 registry 的列表和原文共用 client 的请求门。统一服务的 provider timeout 可缩短
HTTP 等待（至少一秒），但不会超过原准入的 15 秒。

定时采集通过相同 Service handler 执行，逐源串行读取最新列表及选中的原文。
显式运行（PowerShell，父目录需已存在）：

```powershell
cargo run -p magic-market-composition --bin official-news-collector --locked --offline -- --output "C:\DevelopFile\magic-market-data-rs\target\official-news.ndjson" --interval-secs 300 --limit 5
```

默认每轮完成后等待 300 秒，每源最多 5 篇；可选范围为 60..86400 秒与 1..20 篇。
`--rounds 1` 只采集一轮，默认 `--rounds 0` 持续执行，Ctrl+C 停止。
程序不会注册 Windows 任务或自动改动已运行服务。

NDJSON 每行记录一次查询的成功或失败，成功行含完整 canonical envelope；列表成功而
原文失败会保留两行，不将失败当空列表。每行立即 flush，重复观察按轮保留，
无无限增长的内存去重表或并发轮次。输出文件使用独占锁，已有不完整尾行时拒绝追加；
文件错误停止采集。独占锁位于同目录的 `.lock` sidecar，日志仍可被其它程序读取。
操作者负责文件轮换；flush 不承诺断电后的磁盘耐久性，
该日志不证明完整历史。没有启动程序时不会后台访问来源。

## 真实探针与准入证据

在 2026-10-01，八源各完成两轮正式列表/正文读取，以及同一 client 的三次串行
列表读取；证监会每轮额外读取一篇领导活动原文。每轮完整列表均非空，源条数一致，
正文标题/日期与列表相符。首次四源记录见[首批证据](../evidence/2026-10-01-official-news-first-batch.md)，
最终八源记录与追加检查见[追加准入证据](../evidence/2026-10-01-official-news-admission-followup.md)。
Service/gRPC 与定时程序的后续真实验证见
[服务接入证据](../evidence/2026-10-01-official-publication-service-integration.md)。
准入计数登记于 [admissions.tsv](admissions.tsv)，共享传输登记于
[http-transports.tsv](http-transports.tsv)。

正式探针默认受准入门控制：

```bash
cargo run -p magic-official-news-rs --example live_probe --locked --offline -- nbs
cargo run -p magic-official-news-rs --example live_probe --locked --offline -- pbc --serial-load
cargo run -p magic-official-news-rs --example live_probe --locked --offline -- mofcom
cargo run -p magic-official-news-rs --example live_probe --locked --offline -- miit
cargo run -p magic-official-news-rs --example live_probe --locked --offline -- csrc
# 显式诊断不会提升正式准入：
cargo run -p magic-official-news-rs --example live_probe --locked --offline -- gacc --diagnostic
```

使用公开网页不证明自动抓取、展示、再分发权或可用性承诺。部署方按原站条款管理使用。

设计依据：[Gate A](../superpowers/specs/2026-10-01-official-news-and-disclosure-design.md)；
后续合同：[追加 Gate A](../superpowers/specs/2026-10-01-official-news-admission-followup-design.md)；
服务接入设计：[Service Gate A](../superpowers/specs/2026-10-01-official-publication-service-integration-design.md)；
业务合同：[BR-067](../business_rules.md#br-067-official-publication-evidence-and-disclosure-candidates)、
[BR-068](../business_rules.md#br-068-official-publication-service-and-collection-journal)。

## 当前服务部署

2026-10-01，现有 Windows gRPC 实例已更新，八源的列表与原文已通过实际端点验证。
定时采集程序仍需显式启动，尚未常驻。构建身份和上线结果见
[部署证据](../evidence/2026-10-01-official-publication-service-deployment.md)。
