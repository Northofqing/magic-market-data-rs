# 巨潮资讯公告与互动易接入

`magic-cninfo-rs` 是只读的公告/互动问答 Provider，实现
`Announcements` 与 `InvestorQuestions`。它不读取浏览器 Cookie、桌面客户端、
账户、持仓或交易信息。

## 数据源与映射

允许的目标仅为：

```text
https://www.cninfo.com.cn/new/data/szse_stock.json
https://www.cninfo.com.cn/new/hisAnnouncement/query
https://irm.cninfo.com.cn/newircs/index/queryKeyboardInfo
https://irm.cninfo.com.cn/newircs/company/question
https://static.cninfo.com.cn/
```

证券代码先通过巨潮公开映射表解析 `orgId`。映射在进程内缓存 24 小时，并按完整
证券代码精确匹配。公告详情 URL 使用源要求的 `stockCode`、`announcementId`、
`orgId` 和 `announcementTime`；PDF URL 只接受
`static.cninfo.com.cn` 的规范 HTTPS 地址。

## 标准化字段

公告映射为 `Announcement`：

- 公告 ID、标题、公告日期；
- 公告分类（源端未给出时保持 `None`）；
- 规范详情 URL 与 PDF URL；
- 对应证券和完整 `SourceEvidence`。

互动易映射为 `InvestorQuestion`：

- 源问题 ID、问题内容与提问时间；
- 回答内容、回答人和回答时间；
- 对应证券及完整证据；
- 没有真实回答时不会构造回答人或回答时间。

## 分页、限流与错误

- 公共请求最多 300 条；
- 每页最多 30 条，最多自动读取 10 页；
- 单响应最多 8 MiB；
- 默认超时 15 秒；
- 所有客户端克隆共享串行请求门，完整响应读取期间并发为 1；
- 请求起始间隔至少 1 秒；
- 空结果、字段不完整、总数/分页矛盾、非法 URL 或源端错误都返回 typed error。

capability 只声明公告和互动问答；个股新闻与全球新闻明确不支持。

## 探针

```bash
cargo run -p magic-cninfo-rs --example live_probe --release --locked --offline

MAGIC_CNINFO_LOAD_REQUESTS=3 \
MAGIC_CNINFO_LOAD_CONCURRENCY=1 \
MAGIC_CNINFO_LOAD_PACING_MS=1000 \
cargo run -p magic-cninfo-rs --example load_probe --release --locked --offline
```

live probe 默认使用华电辽能 `600396.SH` 验证映射和公告，并使用比亚迪
`002594.SZ` 验证互动易。两个批次都打印 provenance、quality 和全部记录；任何
一项失败都会非零退出。load probe 只压公告链路，最多五次，不代表服务 SLA。

## 生产边界

公告和互动问答是内容数据，不应进入 5 秒行情新鲜度门。调用方负责版权、使用条款、
缓存、持久化和重试策略；本 crate 不下载 PDF 文件、不执行后台轮询，也不把网页
可访问性解释为再分发授权。
