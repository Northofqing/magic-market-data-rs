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
| CFFEX | 生产 `FuturesDeliveryCalendar`；诊断 `probe_futures_delivery_calendar` | 生产范围是版本化、内置的 2026 IF/IH/IC/IM 月度交割表，运行时零网络；明文通知诊断仍可显式运行 | 生产只接受 2026，最后交易日=交割日，方式=`Cash`；其他年份在 I/O 前 `Unsupported`，明文 HTTP 结果永不提升生产表 |

## 端点和请求边界

SSE、SZSE 和 HKEX 正式网络路径只允许以下 exact HTTPS 路径。CFFEX 的
`FuturesDeliveryCalendar` 正式路径不访问网络；以下 CFFEX HTTPS/HTTP 通知路径只供
隔离诊断使用：

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

CFFEX 另有一个 BR-051 限定的诊断例外，只允许 `http://www.cffex.com.cn` 的同组三类
通知路径。它只发送无 body 的 GET，禁止 Cookie、Authorization、代理、跳转、查询参数、
fragment、非 80 端口和端点覆盖。请求合同在联网前校验；响应要求状态 200、最终 URL
精确不变、`text/html` 和 8 MiB 上限。该例外不能用于正式 capability。

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

CFFEX 正式 Provider 使用仓库内版本 `cffex-equity-index-delivery-2026-v1`，运行时不
访问通知站点，也不根据“第三个周五”或本地交易日历计算日期。每个月精确返回 IF、IH、
IC、IM 四个合约，`last_trading_date` 与 `delivery_date` 均取下表固定日期，交割方式为
`Cash`：

| 月份 | 日期 | 月份 | 日期 | 月份 | 日期 |
| --- | --- | --- | --- | --- | --- |
| 01 | 2026-01-16 | 05 | 2026-05-15 | 09 | 2026-09-18 |
| 02 | 2026-02-24 | 06 | 2026-06-22 | 10 | 2026-10-16 |
| 03 | 2026-03-20 | 07 | 2026-07-17 | 11 | 2026-11-20 |
| 04 | 2026-04-17 | 08 | 2026-08-21 | 12 | 2026-12-18 |

批次身份绑定固定表版本与月份；本地 Asia/Shanghai 当前时间只写入
`observed_at`，批次和记录均不伪造 `source_at`。`notice_url` 是官方 HTTPS 目录的
规范引用，不表示本次响应发生了网络抓取。2025、2027 及后续年份都必须先新增经审核
的版本化表和测试，不能在运行时自动外推。

2026-08-18 的官方事实审核使用 CFFEX 的
[IF](https://www.cffex.com.cn/hs300/)、
[IC](https://www.cffex.com.cn/cn/zz500.html)、
[IM](https://www.cffex.com.cn/zz1000/) 和
[IH](https://www.cffex.com.cn/cn/sz50gzqh.html) 产品表；四者均明确最后交易日为到期月
第三个周五、法定假日顺延、交割日同最后交易日、现金交割。
[2026 年休市通知](https://www.cffex.com.cn/jystz/20251217/46425.html) 进一步证明
春节休市至 2 月 23 日并于 2 月 24 日恢复、端午休市至 6 月 21 日并于 6 月 22 日恢复。
逐月核对后得到上表 12 个日期，包含 2 月和 6 月的顺延。运行时仍只读取审核后的结果，
不重新执行公式；未来规则或休市安排变化必须产生新表版本。

独立的 CFFEX 通知诊断仍会在官方交易通知目录中有界扫描最多 120 页，解析同站详情，
并要求标题精确对应请求年月。详情必须同时明确 IF、IH、IC、IM 合约、交割日及交割
结算价措辞，才输出四条事件。通知发布日期写入 source-time evidence；交割日只作为
事件字段。通知未独立说明最后交易日时该字段留空，未独立说明交割方式时使用
`NotProvided`。明文诊断响应中的实际 fetch 模式写入 provenance 和 batch ID；事件的
`notice_url` 只保存同 host/path 的 canonical HTTPS 引用，不能反向解释为 HTTPS 抓取
证据，也不能改写正式固定表。

## 传输与部署

生产 transport 禁止凭据、非 443 端口和跳转，校验最终 URL、精确
JSON/JavaScript media type、8 MiB 上限和 1–60 秒超时。每个客户端 clone 共享串行
请求门，完整响应读取期间不释放，请求起始至少间隔 1 秒。

CFFEX 正式 trait 完全不调用 client transport。通知诊断默认使用 HTTPS；明文诊断探针
必须显式构造 `CffexConfig::plaintext_http_diagnostic()`，不做 TLS fallback，也不读取
浏览器状态。历史 Rustls/Native TLS 失败证据继续保留，用于说明为何通知抓取只保留为
诊断，而不是正式日历的运行时依赖。

SSE、SZSE、HKEX 正式网络能力需要下列前三个 443 出站目标。CFFEX 正式固定表不需要
出站网络；只有主动启用 CFFEX HTTPS 通知诊断时才需要
`www.cffex.com.cn:443`，启用明文诊断时则只对 `www.cffex.com.cn:80` 放行上述固定
路径：

```text
query.sse.com.cn
www.szse.cn
www.hkex.com.hk
www.cffex.com.cn
```

不读取浏览器 Cookie、账户、交易终端或本地行情文件。CFFEX 明文 HTTP 是隔离的显式
诊断模式，不是 HTTPS 请求失败后的自动降级。
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

CFFEX 通知诊断探针可用 `MAGIC_CFFEX_DELIVERY_YEAR` 和
`MAGIC_CFFEX_DELIVERY_MONTH` 覆盖默认的 `2026-02`，并要求精确四条事件。只验证
CFFEX 通知路径、避免其他交易所网络状态影响时使用：

```bash
MAGIC_EXCHANGE_LIVE_OPERATION=cffex-delivery \
MAGIC_CFFEX_DELIVERY_YEAR=2026 \
MAGIC_CFFEX_DELIVERY_MONTH=7 \
cargo run -p magic-exchange-rs --example live_probe --release --locked --offline
```

该现有命令调用的是 `probe_futures_delivery_calendar`，不验收正式固定表。正式 trait
或 gRPC 探针必须另外验证零 transport 调用、四条记录、月份合同代码、固定日期、
`Cash`、共同批次身份和无 `source_at`。非 2026 请求必须在 I/O 前返回 typed
`Unsupported`。

2026-08-18 的正式固定表 Gate C 已完成。两次独立 live 分别验证
`2026-08-21` 和 `2026-02-24`；三次串行调用分别验证 `2026-06-22`、
`2026-01-16` 和 `2026-12-18`。每次都精确返回 IF/IH/IC/IM 四个产品，方式为
`Cash`，批次 strict complete，且 transport 调用次数为零。因此
`CFFEX_2026_FUTURES_DELIVERY_ADMITTED=true`，注册表记录 2 live + 3 serial。
确定性单测和通知诊断仍不能替代这组正式计数。

明文通知诊断继续输出 `diagnostic_probe_status=passed` 与
`admission_state=diagnostic_complete_unadmitted`。其确定性测试仍验证通知四条记录、
节假日顺延日期、`NotProvided` 方法、缺失最后交易日和通知发布日期证据。

2026-07-27 对 `2026-07` 的最新双 backend 准入结果：

```text
rustls:
  tls connection init failed: unexpected end of file
native-tls:
  native_tls connect failed: connection closed via error
```

两次均未取得官方目录 HTTP 响应，因此没有通过运行时 HTTPS 证明
`IF2607/IH2607/IC2607/IM2607` 及其交割日期。这项失败只阻止通知网络路径成为正式
依赖；版本化 2026 固定表不执行该请求。精确命令、时间和完整结果见
[`2026-07-27-cffex-delivery.md`](../evidence/2026-07-27-cffex-delivery.md)。

2026-08-16 使用独立 release 构建再次请求同一 `2026-07` 范围，Rustls 和启用
`native-tls` feature 的系统 TLS 均在精确
`https://www.cffex.com.cn/cn/jystz.html` 建连阶段超时。两次都未取得 HTTP
状态或响应体，因此仍是正式 HTTPS 可达性阻塞，不是解析失败。

同日按 BR-051 启用固定明文 HTTP 诊断后，release 探针返回 4 条完整记录：

```text
IF2607 / IH2607 / IC2607 / IM2607
delivery_date=2026-07-17 source_at=2026-07-17
provenance.source=cffex-official-notice-plaintext-http-diagnostic
diagnostic_probe_status=passed
admission_state=diagnostic_complete_unadmitted
```

详情来自 `/cn/jystz/20260717/48292.html`；记录只保留 canonical HTTPS 引用。由于实际
传输为明文、最后交易日和交割方式仍未被独立证明，该响应继续只是诊断证据，不能用于
补写或动态覆盖正式固定表。

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
