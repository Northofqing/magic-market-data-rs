# 交易所官方数据接入

`magic-exchange-rs` 将上交所、深交所、港交所和中金所保留为独立的一手来源身份。
下表区分真实验收与当前仅通过确定性诊断测试的能力：

| 来源 | 标准化入口 | 验收状态 | 明确边界 |
| --- | --- | --- | --- |
| SSE | `Announcements` | 华电辽能 `600396` 非空公告 | 提供官方 PDF URL metadata，不声明 CDN 下载 SLA |
| SSE | `DragonTigerData` | `600396 / 2026-07-22`：3 条榜单；首条完整买五卖五 | 公开交易信息，不等于 Level-2 |
| SZSE | `Announcements` | 五粮液 `000858` 非空公告 | 详情/PDF URL 严格校验 |
| SZSE | `RealtimeQuotes`、`OrderBooks` | `000858`：Quote + 完整五档 | 数量按源端“手”保留；不推断集合竞价 |
| SZSE | `DragonTigerData` | `000603 / 2026-07-23`：2 条榜单；首条完整买五卖五 | 完整拉取列表分页，详情按完整席位组返回 |
| HKEX | `NorthboundDailyStatistics` | `2026-07-22` 沪股通/深股通各 1 条及各自 Top10 | quota 的 `999,999,999` 哨兵保留为 `Unavailable`，不猜测余额 |
| CFFEX | 诊断 `probe_futures_delivery_calendar`；生产 trait 当前 `Unsupported` | 确定性诊断测试通过；2026-07-25 live 在官方目录 TLS 初始化时收到 unexpected EOF | production capability 为 false；只接受通知明确写出的日期与交割结算价；方式未被事件通知独立证明时保留 `NotProvided`，不按“第三个周五”推算 |

## 端点和请求边界

仅允许以下 exact HTTPS 路径：

```text
query.sse.com.cn/security/stock/queryCompanyBulletin.do
query.sse.com.cn/infodisplay/showTradePublicFile.do
www.szse.cn/api/disc/announcement/annList
www.szse.cn/api/market/ssjjhq/getTimeData
www.szse.cn/api/report/ShowReport/data
www.hkex.com.hk/eng/csm/DailyStat/data_tab_daily_<YYYYMMDD>e.js
www.cffex.com.cn/jystz/
www.cffex.com.cn/jystz/index_<N>.html
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
精确对应请求年月。详情必须同时明确 IF、IH、IC、IM 合约、同一最后交易/交割日及
交割结算价，才输出四条事件。通知未独立说明交割方式时，标准化记录明确使用
`NotProvided`，不会从“交割结算价”推导现金交割。节假日顺延由通知原文证明；
公式计算或交易日历猜测均不准入。根据 BR-009，live 验收通过前
`calendar_capabilities().futures_delivery` 保持 false，生产 trait 返回 typed
`Unsupported`；诊断入口只用于完成该验收，不表示能力已准入。

## 传输与部署

生产 transport 禁止凭据、非 443 端口和跳转，校验最终 URL、精确
JSON/JavaScript media type、8 MiB 上限和 1–60 秒超时。每个客户端 clone 共享串行
请求门，完整响应读取期间不释放，请求起始至少间隔 1 秒。

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

CFFEX 探针可用 `MAGIC_CFFEX_DELIVERY_YEAR` 和
`MAGIC_CFFEX_DELIVERY_MONTH` 覆盖默认的 `2026-02`，并要求精确四条事件。只验收
CFFEX、避免其他交易所的网络状态阻断时使用：

```bash
MAGIC_EXCHANGE_LIVE_OPERATION=cffex-delivery \
MAGIC_CFFEX_DELIVERY_YEAR=2026 \
MAGIC_CFFEX_DELIVERY_MONTH=2 \
cargo run -p magic-exchange-rs --example live_probe --release --locked --offline
```

默认测试证券/日期和覆盖变量见
[`crates/magic-exchange-rs/README.md`](../../crates/magic-exchange-rs/README.md)。

2026-07-23 最终生产 trait 真实结果：

```text
SSE announcements=3 dragon_tiger_entries=3 dragon_tiger_seats=10
SZSE announcements=3 quotes=1 order_books=1
SZSE dragon_tiger_entries=2 dragon_tiger_seats=10
HKEX northbound_daily: Shanghai=1 Shenzhen=1, each Top10=10
live_probe_status=passed

attempts=8 successes=8 failures=0
measurement_elapsed_ms_excluding_output=7510
operation_elapsed_total_ms=2771 pacing_wait_total_ms=4738
attempt_throughput_per_second=1.0652
attempt_latency_min_ms=36 attempt_latency_p50_ms=120
attempt_latency_p95_ms=1201 attempt_latency_p99_ms=1201
attempt_latency_max_ms=1201 minimum_attempt_start_gap_ms=1000
load_probe_status=passed
```

上面的真实结果覆盖 SSE、SZSE 和 HKEX。CFFEX 的确定性诊断测试精确验证
IF2602/IH2602/IC2602/IM2602 四条记录、节假日顺延日期和 `NotProvided` 方法；
2026-07-25 在沙箱内外运行独立命令都在官方目录 TLS 初始化时返回 unexpected EOF，
因此当前只声明诊断实现/合同测试通过，生产 capability 为 false、trait 返回
`Unsupported`，不声明 live 通过。

这里的吞吐是高层数据族 attempt，不是 HTTP RPS；分页/详情会在一个 attempt 内产生
多个受同一限流门约束的请求。数字只证明本次连通、解析、证据和限流行为，不构成
交易所 SLA 或持续抓取许可。

## 仍未接入

- SSE Quote：已观察的公网 host 仍要求旧 TLS，保持 `Unsupported`；
- SSE/SZSE 集合竞价、逐笔委托和 Level-2；
- HKEX 实时北向流、历史回补目录和其他市场数据族。
