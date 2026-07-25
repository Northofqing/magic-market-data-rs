# 国务院政策文件接入

`magic-gov-rs` 是只读、无账号的官方政策 Provider，实现
`PolicyDocuments`。生产请求只允许：

```text
GET https://sousuo.www.gov.cn/search-gov/data
```

请求固定使用政策库类型、按发布时间倒序、标题/正文/摘要检索，并支持受检关键词、
完整起止日期、页码和每页数量。页大小上限为 50，响应上限为 8 MiB，禁止跳转，
客户端克隆共享串行请求门且请求起始至少间隔一秒。

只接收响应中的 `gongwen` 与 `bumenfile` 分类，并要求分类身份与源字段一致。每条
记录保留政策 ID、标题、摘要、发布机构、文号、分类、发布日期、规范链接和
`SourceEvidence`；规范链接必须是 `https://www.gov.cn/` 下的官方文件。错误状态、
缺字段、重复 ID、非法日期、越界日期、非官方 URL、空批次或超量响应都会显式失败。

真实探针：

```bash
MAGIC_GOV_POLICY_QUERY=金融 \
MAGIC_GOV_POLICY_LIMIT=5 \
cargo run -p magic-gov-rs --example live_probe --release --locked --offline
```

探针要求非空严格批次并打印全部来源证据。公共搜索端点没有本项目可证明的 SLA 或
再分发许可；部署方仍负责使用条款、缓存、熔断和监控。
