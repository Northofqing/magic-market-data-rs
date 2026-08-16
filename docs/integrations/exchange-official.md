# 交易所官方数据接入

`magic-exchange-rs` 将上交所、深交所、港交所和中金所保留为独立的一手来源身份。下表
区分已通过生产 trait 真实验收和当前仅可诊断的能力：

| 来源 | 标准化入口 | 真实验收 | 明确边界 |
| --- | --- | --- | --- |
| SSE | `Announcements` | 华电辽能 `600396` 非空公告 | 提供官方 PDF URL metadata，不声明 CDN 下载 SLA |
| SSE | `DragonTigerData` | `600396 / 2026-07-22`：3 条榜单；首条完整买五卖五 | 公开交易信息，不等于 Level-2 |
| SZSE | `Announcements` | 五粮液 `000858` 非空公告 | 详情/PDF URL 严格校验 |
| SZSE | `RealtimeQuotes`、`OrderBooks` | `000858`：Quote + 完整五档 | 数量按源端“手”保留；不推断集合竞价 |
| SZSE | `DragonTigerData` | `000603 / 2026-07-23`：2 条榜单；首条完整买五卖五 | 完整拉取列表分页，详情按完整席位组返回 |
| HKEX | `NorthboundDailyStatistics` | `2026-07-22` 沪股通/深股通各 1 条及各自 Top10 | quota 的 `999,999,999` 哨兵保留为 `Unavailable`，不猜测余额 |
| CFFEX | 诊断 `probe_futures_delivery_calendar`；生产 trait 当前 `Unsupported` | 确定性解析 IF/IH/IC/IM；2026-07-27 Rustls 与 Native TLS 均在官方目录握手失败，未取得 HTTP 响应 | capability 为 false；方式未被通知独立证明时保留 `NotProvided`，不按“第三个周五”推算 |

## 端点和请求边界

仅允许以下 exact HTTPS 路径：

```text
query.sse.com.cn/security/stock/queryCompanyBulletin.do
query.sse.com.cn/infodisplay/showTradePublicFile.do
www.szse.cn/api/disc/announcement/annList
www.szse.cn/api/market/ssjjhq/getTimeData
www.szse.cn/api/report/ShowReport/data
www.hkex.com.hk/eng/csm/DailyStat/data_tab_daily_<YYYYMMDD>e.js
www.cffex.com.cn/cn/jystz.html
www.cffex.com.cn/cn/jystz_<N>.html
www.cffex.com.cn/cn/jystz/<YYYYMMDD>/<ID>.html
```

公告固定按完整远程页校验后本地截断。SZSE 龙虎榜完整读取源端声明的所有页面，要求
分页总数不漂移、累计数量精确匹配且 entry ID 全局唯一；席位详情必须同时包含买一至
买五和卖一至卖五。Router 会二次校验证券、交易日、limit、entry ID 和
`entry/side/rank` 唯一性，错误 strict batch 不能通过切源门。

SZSE `getTimeData` 同时映射 Quote 和五档，核对证券身份、交易阶段、源时间、
OHLC/delta、十档顺序、非锁盘/非交叉和可见总量。源数量保持原始“手”；因为 Core
`Quantity` 暂无单位字段，禁止无证据乘 100。缺少尾档时记录状态为
`Unavailable`，批次质量不完整。

HKEX DailyStat 映射两个北向通道的 CNY 成交额、成交笔数、ETF 成交额和严格 Top10。
证券代码补足六位，超过 JavaScript 精确整数范围的计数、负金额、日期/通道错配、
非 JavaScript MIME 或不完整 Top10 都会失败。

CFFEX Provider 在官方交易通知目录中有界扫描最多 120 页，解析同站详情，并要求标题
精确对应请求年月。详情必须同时明确 IF、IH、IC、IM 合约、交割日及交割结算价措辞，
才输出四条事件。通知发布日期写入 source-time evidence；交割日只作为事件字段。
通知未独立说明最后交易日时该字段留空，未独立说明交割方式时使用 `NotProvided`，
不会从交割日或交割结算价推导其他事实。节假日顺延由通知原文证明；公式计算或交易
日历猜测均不准入。BR-009 live 验收通过前 capability 保持 false，生产 trait 返回
typed `Unsupported`。

## 传输与部署

生产 transport 禁止凭据、非 443 端口和跳转，校验最终 URL、精确
JSON/JavaScript media type、8 MiB 上限和 1–60 秒超时。每个客户端 clone 共享串行
请求门，完整响应读取期间不释放，请求起始至少间隔 1 秒。

CFFEX 诊断 transport 必须由操作方明确选择 `rustls` 或 `native-tls`，默认
`rustls`，禁止一次请求失败后静默切换 TLS 实现。两种实现共享相同的 official URL、
超时、无跳转、MIME、body 上限和 pacing 合同。握手/证书类错误携带所选 backend
作为 typed `ExchangeError::Tls` 返回。默认构建只启用纯 Rust 的 Rustls；Native TLS
是可选 crate feature，避免默认 Linux 构建引入 `openssl-sys`。未编译该 feature
却选择 `native-tls` 会在联网前返回 typed `Unsupported`。

部署需要下列 443 出站访问：

```text
query.sse.com.cn
www.szse.cn
www.hkex.com.hk
www.cffex.com.cn
```

不读取浏览器 Cookie、账户、交易终端或本地行情文件，不提供 HTTP/旧 TLS 降级。
公共端点没有生产 SLA、展示权或再分发授权承诺，使用方应自行确认许可。

## 验收命令与结果

仓库跟随当前 Rust stable，不声明固定 MSRV。

```bash
cargo run -p magic-exchange-rs --example live_probe --release --locked --offline

MAGIC_EXCHANGE_LOAD_REQUESTS=8 \
MAGIC_EXCHANGE_LOAD_CONCURRENCY=1 \
MAGIC_EXCHANGE_LOAD_PACING_MS=1000 \
cargo run -p magic-exchange-rs --example load_probe --release --locked --offline
```

CFFEX 诊断探针可用 `MAGIC_CFFEX_DELIVERY_YEAR` 和
`MAGIC_CFFEX_DELIVERY_MONTH` 覆盖默认的 `2026-02`，并要求精确四条事件。只验收
CFFEX、避免其他交易所的网络状态阻断时使用：

```bash
MAGIC_EXCHANGE_LIVE_OPERATION=cffex-delivery \
MAGIC_CFFEX_DELIVERY_YEAR=2026 \
MAGIC_CFFEX_DELIVERY_MONTH=2 \
MAGIC_CFFEX_TLS_BACKEND=rustls \
cargo run -p magic-exchange-rs --example live_probe --release --locked --offline
```

若要显式诊断系统 TLS backend，将 `MAGIC_CFFEX_TLS_BACKEND` 改为
`native-tls`，并给 Cargo 增加 `--features native-tls`；其他值会在发起网络请求前
失败。Linux 使用该可选 feature 时需要系统 OpenSSL 开发库；默认 Rustls 构建没有此
要求。

该命令只验证诊断实现，不表示生产 capability 已准入。成功标记必须是
`diagnostic_probe_status=passed` 与
`admission_state=diagnostic_complete_unadmitted`，不得输出生产
`live_probe_status=passed`。确定性测试精确验证四条记录、节假日顺延日期、
`NotProvided` 方法、缺失最后交易日和通知发布日期证据。

2026-07-27 对 `2026-07` 的最新双 backend 准入结果：

```text
rustls:
  tls connection init failed: unexpected end of file
native-tls:
  native_tls connect failed: connection closed via error
```

两次均未取得官方目录 HTTP 响应，因此没有证明 `IF2607/IH2607/IC2607/IM2607`
及其交割日期，`calendar_capabilities.futures_delivery` 继续为 `false`，正式 trait
继续返回 typed `Unsupported`。精确命令、时间和完整结果见
[`2026-07-27-cffex-delivery.md`](../evidence/2026-07-27-cffex-delivery.md)。

2026-08-16 使用独立 release 构建再次请求同一 `2026-07` 范围，Rustls 和启用
`native-tls` feature 的系统 TLS 均在精确
`https://www.cffex.com.cn/cn/jystz.html` 建连阶段超时。两次都未取得 HTTP
状态或响应体，因此仍是上游可达性阻塞，不是解析失败；实现没有改用明文 HTTP、
浏览器 Cookie、备用域名或另一来源。

默认测试证券/日期和覆盖变量见
[`crates/magic-exchange-rs/README.md`](../../crates/magic-exchange-rs/README.md)。

2026-07-27 当前树最终生产 trait 真实结果：

```text
SSE announcements=3 dragon_tiger_entries=3 dragon_tiger_seats=10
SZSE announcements=3 quotes=1 order_books=1
SZSE dragon_tiger_entries=2 dragon_tiger_seats=10
HKEX northbound_daily: Shanghai=1 Shenzhen=1, each Top10=10
live_probe_status=passed

attempts=8 successes=8 failures=0
measurement_elapsed_ms_excluding_output=7423
operation_elapsed_total_ms=2697 pacing_wait_total_ms=4726
attempt_throughput_per_second=1.0776
attempt_latency_min_ms=37 attempt_latency_p50_ms=137
attempt_latency_p95_ms=1203 attempt_latency_p99_ms=1203
attempt_latency_max_ms=1203 minimum_attempt_start_gap_ms=1001
load_probe_status=passed
```

这里的吞吐是高层数据族 attempt，不是 HTTP RPS；分页/详情会在一个 attempt 内产生
多个受同一限流门约束的请求。数字只证明本次连通、解析、证据和限流行为，不构成
交易所 SLA 或持续抓取许可。

## 仍未接入

- SSE Quote：已观察的公网 host 仍要求旧 TLS，保持 `Unsupported`；
- SSE/SZSE 集合竞价、逐笔委托和 Level-2；
- HKEX 实时北向流、历史回补目录和其他市场数据族。
