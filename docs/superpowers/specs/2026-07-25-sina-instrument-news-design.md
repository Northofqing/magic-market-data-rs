# Sina 官方个股新闻 Provider 设计

状态：父任务已确认废弃失效的 `feed.mix` 个股参数，采用本设计。

## 1. 目标与边界

`magic-sina-rs` 实现 `NewsProvider::instrument_news`，只读取新浪服务端明确绑定
一个合法 A 股 `InstrumentId` 的官方公司资讯页。首期不实现全局新闻，也不抓文章
正文，不从标题、摘要或正文猜测证券身份。

允许的请求形状只有：

```text
https://vip.stock.finance.sina.com.cn/corp/view/vCB_AllNewsStock.php
    ?symbol=<sh|sz><六位代码>
    &Page=<1..5>
```

URL 由经过校验的请求证券生成，不接受调用方 URL、重定向目标或页面内下一页 URL。
北京证券在当前页面合同完成独立真实验证前返回 `Unsupported`。

## 2. 来源选择与旧模块关系

2026-07-25 真实探针证明旧
`feed.mix.sina.com.cn/api/roll/get?pageid=155&lid=2516&k=<code>` 返回业务状态
`code=11`，已经失效。工作中的 `pageid=153` 返回全局财经流并忽略证券代码，不能
作为个股来源。两者均拒绝。

当前 AllNewsStock 页面同时提供：

- URL 中的精确交易所前缀证券；
- 服务端 `page_symbol = "<symbol>"` 标记；
- 公司资讯专属 `datelist`；
- 每条完整的 `YYYY-MM-DD HH:MM` 发布时间；
- HTTPS 文章 URL。

下游旧 `stock_analysis/src/data_provider/sina_news_provider.rs` 只作为迁移清单：
采用其“新浪个股新闻”业务用途，拒绝旧 endpoint、空批成功、无 MIME 元数据、
宽松时间和无来源证据等行为。本上游切片不编辑下游文件。

## 3. 请求、传输与编码

请求只接受上海/深圳、资产类别为 Equity、六位 ASCII 数字代码。单次业务请求
`limit` 最大 200；每页最多 50 条，最多访问 5 页，单响应沿用 1 MiB 上限。

HTTP 必须：

- HTTPS；
- connect/read/write timeout 为正；
- redirect 数为 0；
- HTTP 200；
- MIME 为带 `gbk`、`gb2312` 或 `gb18030` charset 的 `text/html`；
- 使用固定 Sina Finance Referer 和明确 User-Agent。

响应按 GB18030 严格解码。空体、非法编码、错误 MIME、错误状态、重定向、
超限体积均显式失败。

为不破坏现有 Quote/K 线夹具，`SnapshotTransport` 增加有默认 Unsupported 实现的
文档响应方法；生产 `HttpsTransport` 返回状态、Content-Type、body 和观测时刻。
现有 `get`/`get_with_referer` 行为保持不变。

发布安全审计不得依赖通用 CSS selector 引擎。当前页面合同只需要一个有界的
`div.datelist > ul` 和其中的 anchor，因此实现使用本 crate 的严格结构解析器：
只定位唯一的 `datelist` 容器及其直接 `ul`，逐个读取 anchor 的 `href`、可见文本
和直接前置发布时间文本，并仅解码页面合同所需的标准 HTML 字符引用。未知实体、
嵌套/未闭合 anchor、重复 `href`、缺失直接前置时间或容器边界矛盾均显式失败。
这样删除 `scraper` 及其已停止维护和未获许可的传递依赖，不通过忽略安全公告或
扩大 copyleft 许可白名单绕过发布门禁。

## 4. 页面身份与记录解析

每页必须且只能有一个公司资讯 `div.datelist > ul`，并包含与请求完全一致的
`page_symbol`。页面还必须包含请求页码标记。缺失、重复或矛盾身份直接拒绝；标题
和正文不参与身份判定。

`datelist` 中每个 anchor 的直接前置文本必须包含一个合法
`YYYY-MM-DD HH:MM`。标准化为 `YYYY-MM-DDTHH:MM:00+08:00`。日期必须不早于
2000-01-01，时间必须有效且不得晚于本次 HTTP 观测时刻。页内及跨页顺序必须
非递增。

每条记录映射为：

- `item_id`：来源 canonical URL；
- `title`：anchor 的非空文本；
- `summary` / `content`：`None`，不得补造；
- `publisher`：`新浪财经`，即官方聚合页面的来源平台；
- `canonical_url`：结构化解析后仅接受 Sina 控制的 host；来源若仍为 `http`，只把
  scheme 升级为 `https`，host/path/query/fragment 不变；拒绝凭据和显式端口；
- `published_at`：来源完整时间；
- `instruments`：仅请求中的精确 `InstrumentId`；
- `language`：`zh-CN`；
- `evidence`：`ProviderId::Sina`、来源发布时间、观测时间、共享 batch ID。

## 5. 分页、过滤、去重和原子性

按来源顺序从 Page 1 顺序读取。每页完成身份、数量、时间和 URL 验证后才参与批次：

1. 以 canonical URL 为业务身份；
2. 完全相同的重复项稳定保留第一次；
3. 相同 URL 但标题或发布时间不同，整批失败；
4. `start` / `end` 按来源日期闭区间过滤；
5. `limit` 在去重和日期过滤后应用；
6. 达到 limit、越过 start、或来源明确没有下一页时停止。

若第 5 页仍显示有下一页且请求尚不能完整判定，显式返回分页边界错误，不把截断结果
冒充完整批次。来源页面必须含非空 `datelist`；缺失或空 `datelist`、乱序、无发布
时间或无 canonical URL 均失败。若来源页面和分页证据完整，只是所有记录均落在请求
闭区间之外，则返回 complete 的零记录批次，保留页面观测时间、batch ID，以及本次
读取到的最新来源记录时间作为 provenance `source_at`。这表示 provider-proven
empty range，不得与协议空页或来源不可用混同。

真实深圳页面仍可能输出 `http://stock.finance.sina.com.cn/...`。2026-07-25 对同一
host/path 的 HTTPS 探针返回 200、GBK HTML 且无重定向，因此适配器可执行上述
scheme-only 升级；不得跟随来源 HTTP URL，也不得对非 Sina host 做同类改写。

## 6. 能力与失败模式

`SinaClient::content_capabilities()` 仅声明：

```text
instrument_news=true
global_news=false
announcements=false
investor_questions=false
```

`global_news` 返回 typed `Unsupported`。网络、HTTP、MIME、页面身份、编码、协议
空页、分页边界、时间、重复冲突和 Core 值校验错误均保留为显式错误，不降级为旧
feed、标题匹配、无来源证据的空成功或模拟数据。

## 7. 测试与发布门

TDD 夹具覆盖 URL 身份、沪深、北京拒绝、MIME/状态/空体/超 limit、页面身份、
时间/未来、日期范围、provider-proven filtered-empty 与协议空页的区别、分页边界、
跨观测时刻稳定重复、冲突重复、Sina URL 和证据一致性。

真实探针固定一只上海和一只深圳证券、limit 不超过 3，只打印证券、发布时间、
canonical URL 和 provenance。通过 crate fmt/test、strict Clippy、文档和合规检查
后才声明能力可用。

发布 CI 还必须执行 `cargo test --workspace --all-targets`，确保共享到 example 的
探针测试夹具也随 Core 结构演进；`cargo deny check` 必须无 advisory ignore。
TLS 客户端所需的公开根证书数据可显式允许其宽松
`CDLA-Permissive-2.0` 许可，但不得借此允许 `scraper` 的 MPL 解析器链。

## 8. 回滚

回滚只删除 `magic-sina-rs` 新闻模块、增量 transport 方法、测试/探针和本设计/BR，
恢复内容能力为未声明；Quote、K 线、财务、期权及其他 Provider 不变。不得恢复已
实证失效的 `feed.mix` 个股参数。

若严格页面解析器不能覆盖真实页面，回滚本次 parser 变更并把
`instrument_news` 能力恢复为未声明；不得恢复安全审计失败的 `scraper` 依赖。
