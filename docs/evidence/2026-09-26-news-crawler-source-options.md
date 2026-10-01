# 新闻与公告来源：RSS、正式接口及爬虫可行性

核查日期：2026-09-26。只核查公开的第一方页面与规范；未使用或验证任何用户密钥。本文是来源准入建议，不是已获许可的声明。

## 结论

爬虫**技术上可以补足发现环节**，但不应把「能访问网页」等同于「可自动抓取、存储并向用户再发布」。优先次序是：正式数据 API／授权传输服务 → 发布方提供的 RSS → 获明确许可的定向网页抓取。默认只建立标题、发布时间、实体、原始 URL 等发现索引，用户点击后回到原站；全文、图片、摘要的保存与展示须逐来源确认许可。`robots.txt` 只规定爬虫访问规则，**不是访问授权或版权许可**，见 [RFC 9309 §1](https://www.rfc-editor.org/rfc/rfc9309.html#section-1)。

| 来源 | 已核实的发现方式 | 建议与阻碍 |
| --- | --- | --- |
| NVIDIA 第一方科技新闻 | [Newsroom RSS 页面](https://nvidianews.nvidia.com/rss)列出 All News、Press Releases 等订阅；[News Archive](https://nvidianews.nvidia.com/news)有关键词、年份筛选和分页。2026-01-05 的 [Rubin 原始发布](https://nvidianews.nvidia.com/news/rubin-platform-ai-supercomputer)仍能通过档案定位。 | 可用作 **Rubin 历史命中的权威原始链接**，但**不建议在获得许可前启用生产爬虫或 RSS 定时采集**：[NVIDIA 站点条款 §3.2(f)、§4](https://www.nvidia.com/en-us/about-nvidia/terms-of-service/)明确限制 robot／scraper／crawler，并未因公开 RSS 授予复制或商业展示许可。先向 NVIDIA 获取书面机器访问和索引／展示范围确认。RSS 也不能替代历史回填。 |
| Meta 第一方科技新闻 | [Newsroom](https://about.fb.com/news/)提供分类与分页；[Meta 德语 Newsroom](https://about.fb.com/de/news/)公开列有 RSS 入口，英文 [`/news/feed/`](https://about.fb.com/news/feed/)响应 RSS 媒体类型，但本次未验证条目数量、历史覆盖或稳定性。官方原文包括 [Muse Spark](https://about.fb.com/news/2026/04/introducing-muse-spark-meta-superintelligence-labs/)（2026-04-08）和 [Muse agent](https://about.fb.com/news/2026/09/introducing-muse-personal-ai-agent/)（2026-09-08），须作为不同事件区分。 | 可作为 **Muse 主题的第一方发现候选**；先核对当前 `robots.txt`、RSS 可用性、站点条款和内容展示许可，再考虑低频增量索引。没有核实公开的历史全文 API；不要假定 RSS 能回填全部旧文，也不要把 Facebook／Instagram 用户内容抓取规则与 Newsroom 页面混为一谈。 |
| 美国上市公司披露（SEC EDGAR） | [SEC 官方 API](https://www.sec.gov/search-filings/edgar-application-programming-interfaces)提供免密钥的公司提交历史、10-K／10-Q 等 XBRL 数据；较旧提交在追加 JSON 文件或 bulk ZIP 中。[开发者资源](https://www.sec.gov/about/developer-resources)还列出公司／表单 RSS、每日和季度索引、历史归档及 Forms 3/4/5 过滤。 | **优先正式 API／索引，不抓网页**。10-K／10-Q 对应年报／季报；Form 4 是持股变动申报，不能把每笔交易一概叫“减持”。遵守 SEC 的 [公平访问要求](https://www.sec.gov/about/developer-resources)：识别自动客户端、按需下载、总速率不超过每秒 10 次；批量回填选 bulk ZIP。 |
| 中国上市公司披露（巨潮／上证） | [巨潮资讯公告页](https://www.cninfo.com.cn/new/commonUrl?url=disclosure%2Flist%2Fnotice)可查原始公告；其 [深证信数据服务平台](https://webapi.cninfo.com.cn/V6.0)公开列有“公告资讯／公告定制／API 文档”。[上证所信息网络公告服务](https://www.sseinfo.com/services/other/announcement/)明确包含年报、半年报、季报及公告摘要；[公告文件接口说明书](https://bsp.sseinfo.com/admin/static/public/2024-06-07/1fb00bf6492f41fa8aafbadd4c7fe19d/%E4%B8%8A%E8%AF%81%E9%87%91%E8%9E%8D%E6%95%B0%E6%8D%AE%E5%85%AC%E5%91%8A%E6%96%87%E4%BB%B6%E6%9C%8D%E5%8A%A1%E8%AF%B4%E6%98%8E%E4%B9%A6.pdf)描述按日元数据与下载接口。 | 继续使用项目现有巨潮公告通道，并评估正式数据服务的访问／再分发合同；不要为补报表、减持而另写交易所页面爬虫。公告标题分类只是候选，减持主体、计划／进展／结果及回购股份减持需看原文判定。 |

## 建议的最小实施边界

1. 建立逐来源准入记录：抓取路径、`robots.txt` 检查时间、条款与书面许可、请求频率、用户代理、允许存储和展示的字段；任一权限未知就不启用该自动采集器。发现记录保留原 URL、发布方、原始发布时间和采集时间。
2. 已获许可的 RSS 做低频增量；历史缺口用被许可的档案／批量 API 回填。按 canonical URL 和发布方原始 ID 去重，区分“未命中”与“来源失败／历史未覆盖”。
3. 搜索时同时匹配实体和主题词（例如 `NVIDIA/英伟达` + `Rubin/Vera Rubin`；`Meta` + `Muse Spark` 或 `Muse agent`），并标注“发布方原文”或“第三方报道”。厂商原文索引无法覆盖未获厂商证实的传闻与媒体独家报道；这部分仍需要另行授权的新闻提供商。
4. 不绕过登录、验证码、反爬或付费墙，不假冒浏览器用户，不从未知网页接口逆向出生产依赖；网站可访问不代表可转载全文。若要发布商业化聚合新闻，取得展示、缓存、历史回填等条款后再申请项目的 Provider／HTTP 准入。

以上是基于公开页面与条款的工程判断，并非法律意见；Meta Newsroom 的具体机器访问和再利用许可、NVIDIA RSS 在其自动抓取限制下的适用范围，仍需权利人确认。
