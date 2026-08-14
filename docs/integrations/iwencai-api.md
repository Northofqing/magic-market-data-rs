# iWencai 授权语义搜索接入

`magic-iwencai-rs` 是只读的授权语义搜索 Provider，实现
`SemanticSearch`。它不会读取浏览器 Cookie、桌面客户端登录态、账号密码或交易
数据。

## 鉴权与网络边界

唯一允许的请求目标是：

```text
https://openapi.iwencai.com/v1/comprehensive/search
```

调用方必须通过 `MAGIC_IWENCAI_API_KEY` 提供获授权的 SkillHub API Key；
`IWENCAI_API_KEY` 仅作为兼容别名。Key 只进入请求头，不进入日志、错误正文、
`SourceEvidence` 或探针输出。

缺少 Key、HTTP 401/403 或 API 层拒绝均返回 typed `Authentication` 错误，不会
回退到 Cookie 抓取或模拟结果。传输层只允许
`openapi.iwencai.com:443`、禁止跳转；成功响应必须是 HTTP 200 JSON，单响应最多
4 MiB。所有客户端 clone 共享一个串行请求门，生产请求开始时间至少相隔 1 秒，
并持有到完整响应读取结束。

## 请求与标准化字段

- 查询文本必须非空；
- 返回上限最大 50；
- 相同 document ID 只保留源分数最高的片段；
- 映射文档 ID、频道、标题、摘要、规范 HTTPS URL、可用发布时间和完整证据；
- `observed_at` 在 POST 响应完整读取后采集，不早于响应完成时间；
- 未经验证的动态列不会塞进固定 Core 字段。

公开的 typed `semantic_search` 方法已经实现。2026-08-14 使用获授权 Key 完成两次
release live（每次 7 条）和同一客户端三次串行 load（3/3 成功、共 21 条、最小请求
起始间隔 1000 ms、最大并发 1），因此精确的 `Report` 频道、非空查询、limit ≤ 50
语义搜索范围已准入。路由器仍不会因 fixture 或仅存在 Key 而扩大准入范围。

## 探针

```bash
MAGIC_IWENCAI_API_KEY=... \
  cargo run -p magic-iwencai-rs --example live_probe --release --locked --offline

MAGIC_IWENCAI_API_KEY=... MAGIC_IWENCAI_LOAD_REQUESTS=2 \
  cargo run -p magic-iwencai-rs --example load_probe --release --locked --offline
```

load probe 固定并发 1、由客户端保证请求起始至少间隔 1 秒、最多 3 次，并输出
成功/失败、错误、RPS 和 p50/p95/p99/max。确定性 fixtures 覆盖正常映射、去重、
缺 Key、HTTP 鉴权错误、API 鉴权错误、响应完成后观察时间、Content-Type、
clone 共享门、响应上限和 URL 白名单。真实 probe 不打印 Key；缺少 Key 时仍明确
返回 `SkippedMissingSecret`，已准入不代表可以绕过运行时鉴权。

## 生产边界

API 权限、字段和频率取决于账号授权。本 crate 不绕过鉴权、不保存 Key，也不声明
未通过真实探针的数据族。自然语言查询结果属于盘后研究输入，不应用作 5 秒行情或
交易所事实。
