# MOF、MIIT、NEA 官方原文准入补充研究（2026-10-01）

本轮仅研究财政部、工信部、能源局的官方 HTTPS 来源，没有修改生产代码、基线文件、HTTP registry 或 admission 状态。针对既有混合 HTTP 链接与动态空列表阻碍，找到了三个**范围更窄**、实际源链接可以完整校验的固定入口。它们不是原先全站/最新政策范围的等价替代，也不是新增公共 API 授权证明。

## 结果与范围

| 来源 | 可设计的固定范围 | 完整校验的本轮窗口 | 首两篇正文 | 保留的限制 |
|---|---|---:|---|---|
| MOF | 财政部综合司 `zhs.mof.gov.cn` 的“政策发布”第一页 | 10 条，同主机 HTTPS 相对链接全部通过 | 2 篇均 200 HTML，含实际政策/公告文本 | 彩票、公益金、矿业权等综合司职能；不能声称财政部全站新闻、税政、国债等均覆盖 |
| MIIT | “部领导活动”固定栏目 unit 的第一页 | 24 条，同主机 HTTPS 相对链接全部通过 | 2 篇均 200 HTML | 不是“最新政策”；正文可见发布时间与 `PubDate` 元数据不一致 |
| NEA | “新闻中心”顶部局工作动态的两个静态列表组 | 5 + 5 条，同主机 HTTPS 相对链接全部通过 | 2 篇均 200 HTML | 明确为当前 10 条可见窗口；局工作动态独立栏目、政策栏目仍动态空列表 |

“全部通过”只指声明的固定 DOM/unit 窗口里每条源链接，未丢弃该窗口中的坏行，没有把 HTTP 链接改写成 HTTPS。页面其他导航、宣传、转载版块不属于这些窗口，不能通过扫描全页后只留安全 URL 伪装成整个来源完整。

旧阻碍仍成立：财政部主站“政策发布”和“财政部令”继续给出 HTTP 跨司局链接；工信部“新闻发布会”聚合页仍含 HTTP 跨栏目链接；能源局政策、政府信息公开和独立局工作动态列表仍需 HTTP 前端脚本。相关样本见下文，不能据新范围解除旧范围的失败。

## 方法、证据与请求清单

使用系统默认 TLS 校验的 `Invoke-WebRequest`，有限只读 GET，`TimeoutSec=15`、`MaximumRedirection=0`、UA `magic-market-data-source-research/2026-10-01`；无账户、Cookie、认证或绕过。没有请求 HTTP URL，没有跟随重定向，没有猜测并请求 HTTP 文章的 HTTPS 等价 URL。

21 次有限 GET 均获得 HTTP 200；这里的成功只证明当次 HTTP/解析证据，不证明长期 SLA、全量历史、版权再利用或公共 API 合同。入口由已读取的官方 HTTPS 页面中的相对/明确 HTTPS 链接，以及检索可见的官方 HTTPS 栏目定位；列表里的文章严格按原始相对链接与原始源页解析。未开始分页抓取或持续爬取。

证据保存在 `C:\Users\13687\AppData\Local\Temp\codex-official-news-probe\admission-followup\`，没有覆盖上一轮缓存：

- 每项请求的 `*.metadata.json`：请求 URL、HTTP 状态、媒体类型、字符数、meta、标题及布局标记；正文/新 unit 样本还记录了请求开始/完成 UTC 时间。
- `validated-windows.metadata.json`：四个窗口的全部原始 href、解析后的 HTTPS URL、日期标签、异常主机/协议计数、原证据文件 SHA-256。
- 只保存有限列表/脚本壳及每源首两篇原 HTML 供核对选择器，不做批量正文收集；本报告不复制正文全文。
- 临时证据不是长期离线 fixture；正式准入应将必要的精简样本及准入证据另行固化，并按工程规则登记。

| 实际请求 URL | HTTP | Media type | 解码字符数 | 观察 |
|---|---:|---|---:|---|
| `https://www.mof.gov.cn/zhengwuxinxi/zhengcefabu/` | 200 | `text/html` | 17,443 | 主站政策发布，列表 HTTP 跨司局 |
| `https://www.mof.gov.cn/gkml/caizhengwengao/` | 200 | `text/html` | 15,286 | 文告列表相对 PDF，当前 HTML 正文合同不适用 |
| `https://zhs.mof.gov.cn/zhengcefabu/` | 200 | `text/html` | 12,351 | 综合司政策发布，10 条相对 HTML |
| `https://www.mof.gov.cn/gkml/bulinggonggao/czbl/` | 200 | `text/html` | 18,070 | 财政部令，原始链接为 HTTP `tfs.mof.gov.cn` |
| `https://zhs.mof.gov.cn/zhengcefabu/202608/t20260826_3996112.htm` | 200 | `text/html` | 39,807 | 综合司首篇实际公告正文 |
| `https://zhs.mof.gov.cn/zhengcefabu/202608/t20260814_3995456.htm` | 200 | `text/html` | 38,546 | 综合司第二篇，HTML 含管理办法条文 |
| `https://www.miit.gov.cn/xwfb/index.html` | 200 | `text/html; charset=utf-8` | 35,111 | 新闻发布导航，提供固定子栏目相对链接 |
| `https://www.miit.gov.cn/` | 200 | `text/html; charset=utf-8` | 52,643 | 首页提供领导活动原始相对链接 |
| `https://www.miit.gov.cn/xwfb/xwfbh/index.html` | 200 | `text/html; charset=utf-8` | 13,100 | 新闻发布会聚合页，混合 HTTP/相对链接 |
| `https://www.miit.gov.cn/xwfb/bldhd/index.html` | 200 | `text/html; charset=utf-8` | 4,917 | 部领导活动动态 unit 壳 |
| MIIT 下文所列完整 `.../front/page/build/unit?...pageId=d3e2bede1bc045e2875fc7161c01db7d` | 200 | `application/json` | 7,354 | 24 条相对文章链接，`count=2725` |
| `https://www.miit.gov.cn/xwfb/xwfbh/bxwfbh/index.html` | 200 | `text/html; charset=utf-8` | 5,510 | “部新闻发布会”页面只有子栏目导航，未取得文章列表 |
| `https://www.miit.gov.cn/xwfb/bldhd/art/2026/art_c37cffd9af9a4b3aa6379d153ed83620.html` | 200 | `text/html; charset=utf-8` | 7,217 | 领导活动首文，标题/正文/可见时间齐备 |
| `https://www.miit.gov.cn/xwfb/bldhd/art/2026/art_b6f2e487b7a74338aa4999e8fd91ecc2.html` | 200 | `text/html; charset=utf-8` | 7,394 | 第二篇有发布标签与 meta 日期冲突 |
| `https://www.nea.gov.cn/xwzx/index.htm` | 200 | `text/html` | 12,898 | 新闻中心，顶部 2 × 5 条静态窗口 |
| `https://www.nea.gov.cn/webroot2014/index.htm` | 200 | `text/html` | 22,588 | 信息公开 iframe 导航 |
| `https://www.nea.gov.cn/webroot2014/zfxxgkml/index.htm` | 200 | `text/html` | 3,267 | 表格 tbody 动态空壳 |
| `https://www.nea.gov.cn/webroot2014/zfxxgkml/gfxwj.htm` | 200 | `text/html` | 3,267 | 规范性文件表格亦动态空壳 |
| `https://www.nea.gov.cn/20260930/9ca69805239243efad85690747000ff9/c.html` | 200 | `text/html` | 9,828 | 新闻中心首文实际正文 |
| `https://www.nea.gov.cn/20260930/43d82ce9487a4199a0022e76211c3224/c.html` | 200 | `text/html` | 9,572 | 新闻中心第二篇实际正文 |
| `https://www.nea.gov.cn/news/jwzdt.htm` | 200 | `text/html` | 3,447 | 明示“局工作动态”，仍是动态空列表 |

上表字符数不是传输字节数；原始缓存指纹见文末。重定向策略为禁止自动跟随，本轮没有需要重定向才成功的记录。

## MOF：综合司政策发布

官方新范围为 [财政部综合司政策发布](https://zhs.mof.gov.cn/zhengcefabu/)。这是与 `www.mof.gov.cn` 主站不同的固定主机和独立官方司局栏目，不是将主站的 HTTP 跨域链接升级或作为跨域 fallback。主机、栏目、正文路径必须一起变更到明确的新范围。

### 列表契约

```text
source page: https://zhs.mof.gov.cn/zhengcefabu/
row: ul.liBox > li
anchor: a[href][title]
date: span, YYYY-MM-DD
raw href pattern observed: ./YYYYMM/tYYYYMMDD_ID.htm
resolved article host: zhs.mof.gov.cn
resolved article path: /zhengcefabu/YYYYMM/tYYYYMMDD_ID.htm
```

准确选择 `ul.liBox`，不是全页的所有 `li`。本轮该 UL **全部 10 行**均有日期、标题和同主机相对 HTML href；没有被忽略的 HTTP/异主机行。首条原始 href `./202608/t20260826_3996112.htm`，标题“中华人民共和国财政部公告2026年第24号”，列表日期 `2026-08-26`；第二条 `./202608/t20260814_3995456.htm`，标题“财政部关于印发《彩票市场调控资金管理办法》的通知”，日期 `2026-08-14`。

### 正文契约与时间

[首篇](https://zhs.mof.gov.cn/zhengcefabu/202608/t20260826_3996112.htm) 和 [第二篇](https://zhs.mof.gov.cn/zhengcefabu/202608/t20260814_3995456.htm) 均含实际 HTML 正文，非仅附件下载 stub：第一篇约 4,024 个规范化文本字符，正文讨论彩票公益金筹集、分配及使用；第二篇约 4,563 字符，HTML 内含资金管理办法条文及第十六条实施期限。文本长度来自 `.my_doccontent` 中 `TRS_Editor` 到附件区前的有限清理估算，不是 API 字段或长度保证。

```text
title: h2.title_con
body: .my_doccontent .TRS_Editor
visible publication label: .docreltime > span
article title meta: meta[name=ArticleTitle]
other time meta: meta[name=PubDate]
```

| 字段 | 第一篇 | 第二篇 | 建议精度/来源 |
|---|---|---|---|
| 列表 span | `2026-08-26` | `2026-08-14` | Day；列表日期标签 |
| `.docreltime > span` | `发布日期：2026年08月26日` | `发布日期：2026年08月14日` | Day；可见文章发布日期标签 |
| `PubDate` | `2026-08-26 10:22:00` | `2026-08-14 15:32:00` | 元数据秒级文本，单独保留，不能替换可见日级来源 |
| `ContentSource` | 空字符串 | `财政部` | 不可保证每篇元数据都有来源 |

两个样本里的 `.laiyuan` 日期/来源块在 HTML 注释中，不能作为可见字段选择器。第一篇正文还有正式落款日期，不能把落款、发文日期、发布日期自动合并成同一时刻。最稳妥的发布契约使用可见 `.docreltime > span` 与来源标签 origin，并独立保存元数据。未读取 PDF 附件；不能把 HTML 公告中的附件存在解读成已采集附件全文。

### 未解除的主站阻碍

[主站政策发布](https://www.mof.gov.cn/zhengwuxinxi/zhengcefabu/) 本轮 25 条文章均为明确 HTTP 跨司局链接，例如 `http://jrs.mof.gov.cn/zhengcefabu/phjr/202609/t20260929_3998312.htm`。[财政部令](https://www.mof.gov.cn/gkml/bulinggonggao/czbl/) 本轮展示文章亦以明确 HTTP `tfs.mof.gov.cn` 链接发布，未改写或请求。

[财政文告](https://www.mof.gov.cn/gkml/caizhengwengao/) 的文告文件虽为安全相对链接，但目标是 PDF；与本轮 HTML 原文 profile 不同。综合司范围有价值，却不能代表主站财政新闻、税政、国债或金融司文件均已解决。

## MIIT：部领导活动固定 unit

[部领导活动](https://www.miit.gov.cn/xwfb/bldhd/index.html) 页面显示栏目名、官方主站和动态 unit 脚本。所用 `unitbuild.js` 路径与上一轮一致；准确 `pageId` 变更为该栏目的值：

```text
script id = 95e2ec856b494feaaf57aa695b24c97b
src = /cms_files/default/script/AuthorizedRead/unitbuild.js?v=3.1.3-GXB
url = /api-gateway/jpaas-publish-server/front/page/build/unit
queryData = {
  'parseType':'buildstatic',
  'webId':'8d828e408d90447786ddbe128d495e9e',
  'tplSetId':'209741b2109044b5b7695700b2bec37e',
  'pageType':'column',
  'tagId':'右侧内容',
  'editType':'null',
  'pageId':'d3e2bede1bc045e2875fc7161c01db7d'
}
```

实际成功的完整只读 URL：

```text
https://www.miit.gov.cn/api-gateway/jpaas-publish-server/front/page/build/unit?parseType=buildstatic&webId=8d828e408d90447786ddbe128d495e9e&tplSetId=209741b2109044b5b7695700b2bec37e&pageType=column&tagId=%E5%8F%B3%E4%BE%A7%E5%86%85%E5%AE%B9&editType=null&pageId=d3e2bede1bc045e2875fc7161c01db7d
```

HTTP 200 `application/json`、`success=true`、`code="200"`，`data.html` 是 HTML 片段；该窗口 24 条根相对链接均解析到 `https://www.miit.gov.cn/xwfb/bldhd/art/2026/...`。分页源字段 `rows=24`、`count=2725`、`pageNo=1`，仅是第一页元数据。本轮没有翻页；不证明 2,725 条历史已覆盖。具体动态请求仍是官网前端内部调用，不宣传为正式开放 API。

```text
container: div[id="右侧内容"] > div.page-content
row: div[id="右侧内容"] > div.page-content > ul > li.cf
anchor: a.fl[href][title]
date: span.fr, YYYY-MM-DD
title attribute: 完整标题；可见文本可能截断
article host/path: www.miit.gov.cn /xwfb/bldhd/art/YYYY/art_32hex.html
```

首条原 href `/xwfb/bldhd/art/2026/art_c37cffd9af9a4b3aa6379d153ed83620.html`，标题“工业和信息化部举行升国旗仪式 庆祝中华人民共和国成立77周年”，列表 `2026-09-30`。第二条原 href `/xwfb/bldhd/art/2026/art_b6f2e487b7a74338aa4999e8fd91ecc2.html`，标题“工业和信息化系统国际合作工作座谈会在哈尔滨召开”，列表 `2026-09-28`。全部 24 条同主机路径校验，而非只验证要返回的首 N 条。

### 正文和冲突时间

[第一篇](https://www.miit.gov.cn/xwfb/bldhd/art/2026/art_c37cffd9af9a4b3aa6379d153ed83620.html) 与 [第二篇](https://www.miit.gov.cn/xwfb/bldhd/art/2026/art_b6f2e487b7a74338aa4999e8fd91ecc2.html) 的标题和正文均为静态 HTML：

```text
title: h1#con_title
body: div#con_con.ccontent
visible publication label: .cinfo > span#con_time
visible source: .cinfo > span（首个span为con_time；第二个为来源）
meta: ArticleTitle / PubDate / ContentSource / Url / MakeTime
```

| 样本 | 列表日级日期 | 可见 `#con_time` | `PubDate` 元数据 | `MakeTime` |
|---|---|---|---|---|
| 第一篇 | `2026-09-30` | `发布时间：2026-09-30 16:27` | `2026-09-30 16:48` | `2026-10-01 18:04:24` |
| 第二篇 | `2026-09-28` | `发布时间：2026-09-28 19:19` | `2026-09-29 14:46` | `2026-10-01 18:03:56` |

两篇 `PubDate` 都与可见发布时间冲突，第二篇连日期都不同。应以可见 `#con_time` 作为 Minute 精度的发布标签 origin，独立保留列表 Day 和元数据；不要补零秒制造秒级精度，也不要用 `MakeTime` 或 `PubDate` 覆盖可见发布时间。来源可见值分别为“办公厅、工信微报”“国际合作司”。文本未明确写时区；任何时区解释需由准入设计明示，不能从文本凭空推断。

“部新闻发布会”窄页本轮只有导航，未获取文章列表；“新闻发布会”聚合页混合 HTTP 跨栏目与安全相对文章，不能作为完整安全替代。旧“最新政策”HTTP 正文阻碍仍保留。因此本轮能支持的 MIIT 原文范围是部领导活动，不能对外标成最新政策全部已准入。

## NEA：新闻中心顶部局工作动态静态窗口

[新闻中心](https://www.nea.gov.cn/xwzx/index.htm) 的顶部 `xwzx-page01` 在返回 HTML 中已经包含两组各 5 条文章，读取这些行不需要执行页面里的 HTTP JS。两 UL 在同一 `.xwzx-yw-box`，整体右侧容器只有一个共用 `more_dw` 链接，指向 [局工作动态](https://www.nea.gov.cn/news/jwzdt.htm)。本轮目标页的导航确实明示“局工作动态”，但该目标的列表是动态空壳。

因此可声明的名字是“新闻中心顶部局工作动态 10 条可见窗口”。两组 datasource ID 不同，HTML 没有证明后台两个 datasource 的查询完全同源；不能说已验证完整局工作动态分页或两个后台数据集合同。也不能把整个新闻中心里的新华网时政转载、能源媒体报道等混入本 profile。

### 精确选择器

```css
.xwzx-page01 .xwzx-yw-right .xwzx-yw-box > ul.list01[data="datasource:91fb3999d1964141b668e4a4cef4ed98"] > li
.xwzx-page01 .xwzx-yw-right .xwzx-yw-box > ul.list01[data="datasource:64763711c745408cb6f3bc0895f37649"] > li
```

每行 `a[href]` 加 `span.date`，原始日期 `(YYYY-MM-DD)`；原始 href 为 `../YYYYMMDD/32hex/c.html`，从 HTTPS 源页解析到同主机 `/YYYYMMDD/32hex/c.html`。本轮每组全部 5 行、共 10 行都是这种安全相对路径。`li` 的 `id="zhf"` 重复，不能把它当唯一 ID 或只取一个节点。推荐固定父容器和两组 datasource，按文档顺序拼接，校验所有行再限量输出。不能只用泛化的 `ul.list01 li` 抓取页面其他版块。

首条标题“国家能源局举行升国旗仪式 庆祝中华人民共和国成立77周年”，原 href `../20260930/9ca69805239243efad85690747000ff9/c.html`；第二条标题“第五次中法能源对话在北京举办”，原 href `../20260930/43d82ce9487a4199a0022e76211c3224/c.html`，两条日期均 `(2026-09-30)`。

### 正文与标签 origin

[第一篇](https://www.nea.gov.cn/20260930/9ca69805239243efad85690747000ff9/c.html) 和 [第二篇](https://www.nea.gov.cn/20260930/43d82ce9487a4199a0022e76211c3224/c.html) 均有实际段落 HTML，无需加载 HTTP 脚本取得正文：

```text
title: .article-title > .titles
body: #detailContent（实际为span，内含p及图片）
visible publication label: .article-title > span.times
visible source label: .article-title > span.author
```

| 字段 | 第一篇 | 第二篇 | 来源解释 |
|---|---|---|---|
| 可见 `span.times` | `发布时间：2026-09-30` | `发布时间：2026-09-30` | Day；建议发布标签 origin |
| `meta[name=publishdate]` | `2026-09-30` | `2026-09-30` | 小写名，日级元数据，独立字段 |
| `meta[name=PubDate]` | `2026-09-30 14:15:39` | `2026-09-30 13:43:51` | 秒级元数据，不能与可见日级精度混为一谈 |
| 可见 `span.author` | `来源：国家能源局` | `来源：国家能源局` | 实际可见来源 |
| `meta[name=ContentSource]` | `国家能源局` | `国家能源局` | 与可见来源一致 |
| `meta[name=source]` | `新华网` | `新华网` | 与可见来源不同，不能无差别选任意source meta |

可见发布标签应保持 Day，并标明 origin；不要因为页面另有秒级 `PubDate` 就自动提高精度或拼入不确定的首次发布时刻。第二篇正文叙述活动发生于 9 月 29 日，文章可见发布日是 9 月 30 日，这又说明事件日期、文章发布日期必须分别保留。

### 未解除的动态政策/公开目录阻碍

本輪跟随官方公开目录 iframe 的根相对路径取得 `https://www.nea.gov.cn/webroot2014/zfxxgkml/index.htm`，并取得规范性文件页 `.../gfxwj.htm`。两者都返回 200、3,267 字符，但 `tbody#showData0` 是空的，模板预期 `publishUrl`、`showTitle`、`subtitle`、`publishTime`，仍依赖明确 HTTP 的 `nyjcb_jQuery.js`、`nyjcb_vue.min.js`、`nyjcb_dw_es5_2023.js`。模板预期字段不是已经收到的 JSON 契约。

本轮没有请求这些 HTTP 脚本、没有猜测 HTTPS 数据接口，也没有把返回 200 的空壳解释为没有政策。上一轮“最新文件”“政策解读”“新闻发布”的动态阻碍仍适用。新 profile 是已验证的静态新闻窗口，不是政策库或公开目录准入。

## 原证据指纹与采用建议

`validated-windows.metadata.json` 验证结果：MOF 10 条、MIIT 24 条、NEA 两组各 5 条，异常 HTTPS 协议数和异常主机数均为 0。对应原证据 SHA-256：

```text
mof-comprehensive-policy.html
4755667A48492B136DF7AF6CEABC69BECED14C95EE5DF55E122E919E60921319
miit-leadership-unit.json
60A5D6CD0B9EFC46E37C9B68D2EC5EEB760E724ADBF4A695BE7FAB1156833031
nea-news.html（两组共享同一个HTML证据）
8A02AC3A217B68641BEF1CDA11D13AD8BF9A6386862FA15B405609B957C654D6
```

进入正式设计时，应将栏目范围、主机、精确路径、unit 固定参数、全部行校验、日期精度和 `PublicationLabelOrigin` 一起登记。正文发布时间优先使用已证实的可见标签；其他 meta、事件日期、发文落款、页面生成时间作为不同 provenance 保留。未来窗口出现 HTTP/异主机/日期缺失，按明确失败合同暴露，不能静默过滤后装作窗口完整。

本报告的外部证据只来源于上述官方页面和实际官方前端响应。没有代码改动，无需代码测试；正式实现及 Gates A–D 仍由主代理执行。本次研究没有复测 GACC；主代理交接称共享 Rust 原生传输实测仍 `Network` 失败，无 TLS 配置变化，不把它归入本轮三个成功范围。
