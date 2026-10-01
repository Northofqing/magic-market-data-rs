# 国内官方新闻列表入口核验（2026-10-01）

本次只读核验工信部、商务部、证监会、能源局的公开网页及其实际前端列表请求，为国内官方原文候选提供技术证据。没有新增 provider、依赖或准入记录，没有使用账户、验证码绕过、明文请求、TLS 降级或证书校验豁免。海关总署的证书链阻碍来自主审交接，本分支没有重试或绕过。

## 结论与实现边界

| 来源 | 实际核验结果 | 可用于实现的边界 |
|---|---|---|
| 商务部日常新闻发布 | HTTPS 列表前端 GET 返回 JSON 内嵌 HTML；首面及第二面均成功；首两篇 HTTPS 正文成功 | 可以设计固定栏目、固定查询键的动态列表 profile；公开前端调用不等于正式公共 API 授权 |
| 证监会要闻 | HTTPS `searchList` 返回结构化 JSON；首两篇 HTTPS 正文成功 | 可以设计固定 channel 的 JSON profile；保留列表发布时间、文章生成时间与日级展示的区别 |
| 工信部最新政策 | HTTPS 列表 GET 成功，但 24 条样本中 23 条文章链接明确为 HTTP，一条为根相对路径；日期顺序并非严格递减 | 当前不能以 HTTPS 正文完整源准入；不得把 HTTP 链接改写成 HTTPS 后当作源站提供的安全链接 |
| 能源局分类页 | 三个 HTTPS 分类页均 200，但为 Vue 动态空列表；必要脚本明确为 HTTP | 尚未证实 HTTPS 列表数据入口。HTTPS 首页静态锚点只能提供有限发现，不能代表分类全集 |
| 海关总署 | 主审的 Python 默认 TLS 请求出现自签证书链失败 | 保留阻碍；没有新证据证明可直接接入 |

以上是一次有限请求的技术观察，不能证明长期可用、授权许可、全量历史覆盖或更新 SLA。候选采用前仍需按 [admissions.tsv](../integrations/admissions.tsv)、[http-transports.tsv](../integrations/http-transports.tsv) 与工程 Gate A 登记准确的主机、路径、重定向、体积与失败策略。本报告没有更改登记状态。

## 核验方法与证据位置

使用 Windows `Invoke-WebRequest` 发出有限只读 GET，`TimeoutSec=15`、`MaximumRedirection=0`、系统默认证书验证；没有 Cookie、认证头或登录。请求 UA 为 `magic-market-data-source-research/2026-10-01`。先读官方 HTML 中的脚本、`url` 和 `queryData`，再请求页面实际使用的 HTTPS 路径。本次成功样本均没有发生需要跟随的重定向；不能据此推断其他路径也无重定向。

原始证据暂存于 `C:\Users\13687\AppData\Local\Temp\codex-official-news-probe\`，不把临时缓存当长期 fixture：

- 原始列表页面：`miit.html`、`mofcom.html`、`csrc.html`（主审先前抓取，本分支读取）；能源局本分支另存 `nea-listing.html`。
- 实际前端脚本：`miit-listing.js`、`mofcom-listing.js`、`csrc-listing.js`；分页脚本 `miit-pagination.js`、`mofcom-pagination.js`。
- 实际列表响应：`miit-listing-response.json`、`mofcom-listing-response.json`、`mofcom-listing-page2-response.json`、`csrc-listing-response.json`。
- 商务部片段：`mofcom-listing-fragment.html`；正文 `mofcom-article-1.html`、`mofcom-article-2.html`。
- 证监会正文：`csrc-article-1.html`、`csrc-article-2.html`。
- 能源局分类壳：`nea-xwfb-index-htm.html`、`nea-policy-zxwj-htm.html`、`nea-policy-jd-htm.html`。

临时文件保存 UTF-8 内容，不包含账户或认证数据。字节数是保存后 UTF-8 大小，字符数来自客户端解码结果，不冒充原始传输长度。下文 `total`、`count` 均为源站字段样本，不代表本轮抓取数量。

## 商务部：固定 unit GET 已核验

官方页面为 [日常新闻发布](https://www.mofcom.gov.cn/xwfb/rcxwfb/index.html)。其前端不是静态文章列表，而是一个动态 unit：

```text
script id = cb38aba59e094b359a406e13e4ca33ed
src = /cms_files/default/script/AuthorizedRead/unitbuild.js?v=4.3.0-SWB-U19
url = /api-gateway/jpaas-publish-server/front/page/build/unit
queryData.tagId = 分页列表
queryData.pageId = 95d89972d8aa4fcea511701cd0f212d9
```

定位时应校验固定 `url`、`queryData.tagId` 和 `pageId`，而不是把 script 的 ID 误认为“分页列表”。`div#分页列表` 才是响应片段的栏目容器。[官方 unitbuild.js](https://www.mofcom.gov.cn/cms_files/default/script/AuthorizedRead/unitbuild.js?v=4.3.0-SWB-U19) 把 `queryData` 属性的单引号替换为双引号、解析 JSON，然后作为 Ajax GET 的键值对象发送。它不是名为 `queryData` 的单个字符串 query 参数。

首面实际成功请求如下，`bulidstatic` 是源页面的原始拼写：

```text
https://www.mofcom.gov.cn/api-gateway/jpaas-publish-server/front/page/build/unit?parseType=bulidstatic&webId=8f43c7ad3afc411fb56f281724b73708&tplSetId=52551ea0e2c14bca8c84792f7aa37ead&pageType=column&tagId=%E5%88%86%E9%A1%B5%E5%88%97%E8%A1%A8&editType=null&pageId=95d89972d8aa4fcea511701cd0f212d9
```

| query 键 | 源页面值 |
|---|---|
| `parseType` | `bulidstatic` |
| `webId` | `8f43c7ad3afc411fb56f281724b73708` |
| `tplSetId` | `52551ea0e2c14bca8c84792f7aa37ead` |
| `pageType` | `column` |
| `tagId` | `分页列表` |
| `editType` | 字符串 `null` |
| `pageId` | `95d89972d8aa4fcea511701cd0f212d9` |

首面没有 `pageNo` 或 `paramJson` query。观察到 HTTP 200、`application/json; charset=UTF-8`，响应 5,229 字符，JSON 顶层为 `roles`、`permissions`、`success`、`message`、`code`、`data`、`traceId`；`success=true`、`code="200"`，`data.html` 是 HTML 片段。片段为 `div#分页列表 div.page-content ul.txtList_01 > li`，每条 `a[href][title]` 加 `span`，日期形式 `[YYYY-MM-DD]`。15 条样本文章 `href` 均为根相对路径，可以按确切 HTTPS 源站解析，不需要协议改写。

```text
首条 title = 国家金融监督管理总局 商务部发布《关于加强商务和保险协同 做好内贸险有关工作的通知》
href = /xwfb/rcxwfb/art/2026/art_7922bc64c4b7410c9d4a54af10921130.html
日期 = [2026-09-29]
第二条 title = 商务部公布中美贸易理事会和“300亿对300亿”对等降税框架有关情况
href = /xwfb/rcxwfb/art/2026/art_7f879be341ec4137a22f6d0360a5a291.html
日期 = [2026-09-28]
```

### 有限翻页验证

响应 `div.pagination` 的 `rows=15`、`count=8242`、`pageNo=1`、`unitid=分页列表`、`unitUrl=/api-gateway/jpaas-publish-server/front/page/build/unit`。它的 `queryData` 不包含 `editType`。[官方分页脚本](https://www.mofcom.gov.cn/cms_files/default/script/ajax/page/page.js) 在翻页时设置单个 query 参数：

```json
{"paramJson":"{\"pageNo\":2,\"pageSize\":15}"}
```

第二面实际请求并成功：

```text
https://www.mofcom.gov.cn/api-gateway/jpaas-publish-server/front/page/build/unit?webId=8f43c7ad3afc411fb56f281724b73708&pageId=95d89972d8aa4fcea511701cd0f212d9&parseType=bulidstatic&pageType=column&tagId=%E5%88%86%E9%A1%B5%E5%88%97%E8%A1%A8&tplSetId=52551ea0e2c14bca8c84792f7aa37ead&paramJson=%7B%22pageNo%22%3A2%2C%22pageSize%22%3A15%7D
```

HTTP 200、`application/json; charset=UTF-8`，5,243 字符；响应分页 `pageNo=2`、`rows=15`、`count=8242`。首条为“商务部等7部门联合开展报废机动车非法回收拆解专项整治行动”，日期 `[2026-06-24]`、路径 `/xwfb/rcxwfb/art/2026/art_9e910de61fd547f0b68002fdb9c4ff71.html`。本轮只有一面首面和一面翻页，不证明 8,242 条历史完整，也不启用脚本中搜索分支的大页长。

### 正文与时间字段

首两篇正文均 HTTP 200 `text/html`。[首篇正文](https://www.mofcom.gov.cn/xwfb/rcxwfb/art/2026/art_7922bc64c4b7410c9d4a54af10921130.html) 与 [第二篇正文](https://www.mofcom.gov.cn/xwfb/rcxwfb/art/2026/art_7f879be341ec4137a22f6d0360a5a291.html) 共同可用的字段为：

```text
meta[name=ArticleTitle]
meta[name=PubDate]
meta[name=ContentSource]
meta[name=Url]
div.art-con[ergodic=article]  正文
```

首篇 `PubDate=2026-09-29 17:03`、来源“商务部新闻办公室”；第二篇 `PubDate=2026-09-28 11:13`。这是源站分钟级文本，未声明时区，不能凭习惯补到秒或断言 UTC 时刻。第二篇 `MakeTime=2026-09-30 17:00:46` 与发布日不同，应独立保留生成/更新时间来源，不能替换发布日期。

首篇正文指向 `https://cws.mofcom.gov.cn/gztz/art/2026/art_fa3117cbb72a4ce78249719ae66e4fd2.html` 的政策原件。本轮只记录这个明确 HTTPS 外链，没有抓取或扩大 `cws.mofcom.gov.cn` 准入。发布报道与政策原文可随后建立关联，但应分开保留内容性质。

## 证监会：固定 channel 的 JSON 列表已核验

当前 [证监会要闻](https://www.csrc.gov.cn/csrc/c100028/common_xq_list.shtml) 页面通过 [common_xq_list.js](https://www.csrc.gov.cn/csrc/xhtml/js/common_xq_list.js) 请求列表。页面 meta 的 `channelid=a1a078ee0bc54721ab6b148884c784a8`，页长 18。实际成功请求：

```text
https://www.csrc.gov.cn/searchList/a1a078ee0bc54721ab6b148884c784a8?_isAgg=true&_isJson=true&_pageSize=18&_template=index&_rangeTimeGte=&_channelName=&page=1
```

HTTP 200，`application/json;charset=utf-8`，144,084 字符、保存后 281,038 UTF-8 字节；18 条结果。顶层实际字段是 `data`、`locationUrl`、`channelName`，没有已观察到的 `code` 字段。`data.total=3197`、`data.channelId=a1a078ee0bc54721ab6b148884c784a8`、`data.results` 为数组。总数是源站样本，未验证全部页。

```json
{
  "manuscriptId": "7661513",
  "title": "湖北证监局原党委书记、局长王广幼被开除党籍",
  "url": "//www.csrc.gov.cn/csrc/c100028/c7661513/content.shtml",
  "publishedTimeStr": "2026-09-28 15:15:26",
  "publishedTime": 1790550926000,
  "channelName": "证监会要闻",
  "channelCodeName": "c100028"
}
```

第二条 `manuscriptId=7659506`，标题“中国证监会拟对17起案件线索的‘吹哨人’给予奖励”（正文原始标题使用中文双引号），`url=//www.csrc.gov.cn/csrc/c100028/c7659506/content.shtml`、`publishedTimeStr=2026-09-18 15:27:06`、`publishedTime=1789687626000`。协议相对路径在源 HTTPS 页面上下文中解析为 HTTPS，不是把明确的 HTTP 链接升级。

官方脚本只将 `publishedTimeStr.substr(0,10)` 显示为列表日期；底层响应还有秒级文本和 epoch 毫秒。JSON 还含 `contentHtml`、`content`、`memo`、`websiteId` 及 `domainMetaList` 部门/来源等内部字段。应采用固定字段白名单和体积上限，不能因列表已含全文就认为获得了公共全文 API 或再分发许可。脚本中另一段 `/getLocalList?channelCode=...` 是注释掉的面包屑逻辑，本轮没有把它当实际调用或必要 endpoint。

### 发布时间与页面生成时间冲突

[第一篇](https://www.csrc.gov.cn/csrc/c100028/c7661513/content.shtml) 和 [第二篇](https://www.csrc.gov.cn/csrc/c100028/c7659506/content.shtml) 正文均 HTTP 200 `text/html`；正文为 `div.detail-news`，页面日期/来源为 `div.info > p.fl`。文章 `meta[name=ArticleTitle]`、`meta[name=ContentSource]` 可用，但 `meta[name=PubDate]` 不能无条件覆盖列表发布时间：

| 样本 | 列表 `publishedTimeStr` | 文章 `PubDate` | 文章 `others` | 正文可见日期 |
|---|---|---|---|---|
| 7661513 | `2026-09-28 15:15:26` | `2026-09-28 15:35:13` | `页面生成时间 2026-09-28 15:35:13` | `2026-09-28` |
| 7659506 | `2026-09-18 15:27:06` | `2026-09-18 18:17:50` | `页面生成时间 2026-09-18 18:17:50` | `2026-09-18` |

观察证据表明本模板的 `PubDate` 与页面生成时间一致。实现时应保留原始列表 `publishedTime`/`publishedTimeStr` 及其字段来源，另存文章日期与生成时间；不能把元数据中的精确秒数自动认定为新闻首次发布秒数。源 `publishedTime` 为 epoch 毫秒，`publishedTimeStr` 文本未声明时区，两种字段都保留才可检查时间转换。

`common_xq_list.shtml` 是本轮实际动态栏目；不要以旧 `common_list.shtml` 静态归档样本证明当前栏目已覆盖。

## 工信部：HTTPS 列表成功，正文链路仍有明确阻碍

[最新政策](https://www.miit.gov.cn/xwfb/zxzc/index.html) 调用 [官方 unitbuild.js](https://www.miit.gov.cn/cms_files/default/script/AuthorizedRead/unitbuild.js?v=3.1.3-GXB)。实际请求为：

```text
https://www.miit.gov.cn/api-gateway/jpaas-publish-server/front/page/build/unit?parseType=buildstatic&webId=8d828e408d90447786ddbe128d495e9e&tplSetId=209741b2109044b5b7695700b2bec37e&pageType=column&tagId=%E5%8F%B3%E4%BE%A7%E5%86%85%E5%AE%B9&editType=null&pageId=42cd38164cb6441a84bdfa441909e15e
```

HTTP 200 `application/json`、8,518 字符，保存后 12,014 UTF-8 字节。顶层 `roles`、`permissions`、`code`、`success`、`data`、`message`，`code="200"`、`success=true`、`data.html` 为片段。正文列表结构 `div#右侧内容 div.page-content ul > li.cf`，`a.fl` 的 `title` 属性保存完整标题，可见文本可能截断，日期 `span.fr` 为 `YYYY-MM-DD`。

24 条样本，分页 `rows=24`、`count=536`、`pageNo=1`、`unitid=右侧内容`；分页脚本 [page.js](https://www.miit.gov.cn/cms_files/filemanager/script/ajax/page/page.js) 显示可用 `paramJson={pageNo,pageSize}`，但本轮没有请求第二面，也不证明历史覆盖。

首条标题“四部门关于开展2026年度享受增值税加计抵减政策的集成电路企业清单制定工作的通知”，日期 `2026-09-30`，原始链接：

```text
http://www.miit.gov.cn/zwgk/zcwj/wjfb/tz/art/2026/art_7d2e760b4be94217b8f55caec840b30d.html
```

本轮 24 条中 23 条明确为 HTTP，唯一根相对链接为 `/xwfb/zxzc/art/2026/art_f7a75444808e4e3b8f13b7a84ac733e4.html`，日期 `2026-09-04`。没有改写、请求这些 HTTP URL。列表还出现 `2026-09-18` 条目排在 `2026-09-21` 之前，不能把页面前 N 条直接解释成发布时间排序后的最新 N 条。

因此只能说已发现并验证 HTTPS 元数据片段请求，不能说已验证该栏目的 HTTPS 正文完整链路。若后续只做元数据候选，也必须明确每条 URL 的协议阻碍，不能静默丢弃 HTTP 条目然后宣称全栏目完整。

## 能源局：HTTPS 分类页是动态空壳

[能源局 HTTPS 首页](https://www.nea.gov.cn/) 本轮 HTTP 200 `text/html`，99,205 字符，包含静态文章锚点及以下明确 HTTPS 分类链接。本轮各请求一次，均 HTTP 200 `text/html`：

| 页面 | 本轮字符数 | HTML 中 datasource |
|---|---:|---|
| [新闻发布](https://www.nea.gov.cn/xwfb/index.htm) | 3,445 | `4f3484af7ea244e7ab18d094856c82a6`（列表）；另有 `c583b2bf6d3b4ae6a67bc08d61f5ad96` 导航 datasource |
| [最新文件](https://www.nea.gov.cn/policy/zxwj.htm) | 3,445 | `40d365c13659452aa06cdb7268d6192e` |
| [政策解读](https://www.nea.gov.cn/policy/jd.htm) | 3,441 | 以保存的源 HTML 为准，本轮未请求动态数据 |

三个页面的文章 `<ul id="showData0" class="list" ...>` 都是空的。内联逻辑为 `new Xhwpage({parentNode:'#showData0',type:'page',pageSize:25,...})`，模板期望 `datasource.datasource[]` 内 `publishUrl`、`showTitle`、`contentType`、`publishTime`，日期显示为年月日过滤器拼接。**这些是模板字段，尚未收到真实列表 JSON，不能作为已验证响应契约。**

依赖均为源 HTML 明确的 HTTP 路径，例如：

```text
http://www.nea.gov.cn/2015nyj/xinban/js/nyjcb_jQuery.js
http://www.nea.gov.cn/2015nyj/xinban/js/nyjcb_vue.min.js
http://www.nea.gov.cn/2015nyj/xinban/js/nyjcb_dw_es5_2023.js
```

没有请求这些明文脚本，也没有猜测其 HTTPS 等价路径或潜在数据服务路径。因此尚未证实 HTTPS 分类数据端点，不能把 HTTP 200 空 HTML 当作“没有新闻”。首页中政策原文有根相对路径样本 `20260930/43fede537ca048b1a6a51fe0970fb205/c.html`，也有明确 HTTP 文章链接；有限首页锚点发现不能替代完整分类覆盖、日期排序或历史分页。

## 海关总署与准入建议

主审交接记录：GACC 的 Python 默认 TLS 校验出现自签证书链失败。本分支没有复测、禁用证书验证、切换明文、修改根证书或用绕过方式继续。这个阻碍与尚未证明的列表结构应分别保留，不能把阻碍转写为“海关没有公开新闻”。

优先进入设计与实现的是商务部固定 unit profile、证监会固定 channel profile：第一轮限定栏目和有限页数，校验响应类型、必要字段、单位/channel ID、文章域名与路径、日期精度及失败显式暴露。已有 HTML 正文只抓取明示 HTTPS 或 HTTPS 源上下文的相对链接。工信部、能源局和海关暂留研究候选，等待源站明确 HTTPS 正文/脚本/数据入口或可验证证书链证据。前端请求形态不得宣传成已获公共 API 授权。
