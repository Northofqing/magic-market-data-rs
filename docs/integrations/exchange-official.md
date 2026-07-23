# 交易所官方数据接入

`magic-exchange-rs` 把上交所、深交所和港交所保留为独立的一手来源身份。当前只对
SSE/SZSE 官方公告开放 capability；HKEX 只有 `ProviderId::Hkex` 占位，任何尚未
完成契约、fixture 和真实 probe 的数据族都保持关闭。

## 当前能力

| 来源 | 标准化入口 | 真实验收 | 明确边界 |
| --- | --- | --- | --- |
| SSE | `Announcements` | 华电辽能 `600396` 取到 3 条公告 | 官方 PDF URL metadata；未声明下载成功 |
| SZSE | `Announcements` | 五粮液 `000858` 取到 3 条公告 | 抽样详情页 HTTP 200，抽样 PDF 为 `application/pdf` |
| HKEX | 无 | 未验收 | 北向统计、Top10 等后续单独建无损契约 |

SSE 使用官方 JSONP
`https://query.sse.com.cn/security/stock/queryCompanyBulletin.do`；SZSE 使用官方
JSON POST `https://www.szse.cn/api/disc/announcement/annList`。两者固定按 50 条
远程分页，完成页级校验后才按调用方 limit 截断，最多 10 页/500 条。

每一条严格记录都要求：

- 源证券代码存在并与请求证券、交易所一致；
- 源发布日期是有效公历日期，并位于可选请求范围；
- 公告 ID 唯一，分页总数和页序不漂移；
- 详情/PDF URL 使用经过验证的官方 HTTPS host/path；
- record 与 batch 保留独立 Provider、source time、observed time 和 batch ID。

Router 的 `announcement_source` 还会二次检查请求证券、日期范围、调用上限、来源
日期和公告 ID 唯一性；具体 Provider 即使错误返回 strict batch，也不能靠证据名称
相同通过路由。

## 传输与部署

生产 transport 只允许上面两个 exact HTTPS host/path，禁止凭据、非 443 端口和跳转；
校验最终 URL、JSON/JavaScript Content-Type、8 MiB 响应上限及 1–60 秒超时。
每个客户端的 clone 共享串行请求门，完整响应读取期间不释放，生产请求起始至少间隔
1 秒。SSE 和 SZSE 各自限流；load probe 另在跨来源 attempt 层保持至少 1 秒间隔。

部署只需普通 Rust 二进制和下列 443 出站访问：

```text
query.sse.com.cn
www.szse.cn
```

不读取浏览器 Cookie、账户、交易终端或本地行情文件，不提供 HTTP 降级或旧 TLS
兼容模式。

## 验收命令

```bash
RUSTUP_TOOLCHAIN=1.83.0 \
cargo run -p magic-exchange-rs --example live_probe --release --locked --offline

MAGIC_EXCHANGE_LOAD_REQUESTS=4 \
MAGIC_EXCHANGE_LOAD_CONCURRENCY=1 \
MAGIC_EXCHANGE_LOAD_PACING_MS=1000 \
RUSTUP_TOOLCHAIN=1.83.0 \
cargo run -p magic-exchange-rs --example load_probe --release --locked --offline
```

2026-07-23 最终真实结果：

```text
SSE records=3
SZSE records=3
live_probe_status=passed

attempts=4 successes=4 failures=0
measurement_elapsed_ms_excluding_output=4304
operation_elapsed_total_ms=2458
pacing_wait_total_ms=1845
attempt_throughput_per_second=0.9294
attempt_latency_p50_ms=1082
attempt_latency_p95_ms=1214
attempt_latency_p99_ms=1214
attempt_latency_max_ms=1214
minimum_attempt_start_gap_ms=1003
load_probe_status=passed
```

这里的吞吐是高层 announcement attempt，不是 HTTP RPS；分页时一个 attempt 会发送
多次 HTTP 请求。Provider 返回后立即采样，批次/记录的终端输出不进入测量窗口。
负载数字只证明当前连通、解析和限流行为，不构成交易所 SLA 或持续抓取许可。

## 后续官方源切片

- SSE/SZSE 官方龙虎榜列表与席位明细；
- SZSE `getTimeData` Quote/五档（源数量从手精确转换为股）；
- HKEX DailyStat 北向成交额、笔数、ETF 成交额与 Top10 的无损契约；
- SSE Quote 在已观察公网 host 仍要求旧 TLS，保持 `Unsupported`，不增加不安全兼容。
