# 财联社公开电报接入

`magic-cls-rs` 是只读的全球新闻补充 Provider。它实现
`NewsProvider::global_news`，不读取桌面客户端、Cookie、账号或交易数据。

## 数据源与网络边界

唯一允许的请求目标是：

```text
https://www.cls.cn/v1/roll/get_roll_list
```

客户端在本地按排序后的查询串计算 `md5(sha1(query))` 签名，不需要 API Key。
传输层只允许 `www.cls.cn:443`、禁止跳转、连接/读/写均有超时，只接受 HTTP 200
JSON，单响应最多 2 MiB。所有客户端 clone 共享串行请求门，生产请求开始时间至少
相隔 1 秒，并持有到完整响应读取结束。返回体必须满足 `errno == 0`，记录数不能
超过请求上限。

请求的 `rn` 最大为 50。load probe 固定并发 1、请求间隔至少 1 秒、最多 3 次，
不会把公开端点当成高并发生产队列。

## 标准化字段

每条电报映射为 `NewsItem`：

- `item_id`、标题、摘要/正文；
- 发布者、发布时间、规范 HTTPS 链接；
- 源中明确给出的证券和主题；
- `zh-CN` 语言标记；
- `SourceEvidence` 的 provider、源时间、观察时间和批次 ID。

没有证券关联的全球电报保留空证券列表，不猜测代码。按证券过滤的
`instrument_news` 当前明确返回 `Unsupported`。字段缺失或显式 `null` 可以表示
没有证券/主题；但源字段一旦存在，非数组、非对象、空名称、非法市场前缀或非法
六位代码都会返回 `Protocol`，不会静默丢弃后将批次标成完整。

## 探针

```bash
cargo run -p magic-cls-rs --example live_probe --release --locked --offline
MAGIC_CLS_LOAD_REQUESTS=2 \
  cargo run -p magic-cls-rs --example load_probe --release --locked --offline
```

live probe 打印批次证据和每条 `NewsItem` 的全部字段；空响应、错误码、非法 URL
或字段错误都会非零退出。load probe 最多三次，输出成功/失败、错误、RPS 和
p50/p95/p99/max。确定性 fixtures 已覆盖签名、错误码、Content-Type、URL
白名单、严格证券/主题字段和 clone 共享门。2026-07-23 真实 probe 已返回 5 条
完整电报并通过；真实负载结果详见 `docs/PERFORMANCE_RESULTS.md`。

## 生产边界

公开网页端点没有本项目可证明的 SLA 或再分发许可，只适合作为盘后/研究补充源。
调用方负责持久化、调度、缓存和版权合规；本 crate 不运行后台线程或推送服务。
