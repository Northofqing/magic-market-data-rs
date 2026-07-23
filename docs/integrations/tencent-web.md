# 腾讯网页行情补充源

## 定位与边界

`magic-tencent-rs` 只读访问腾讯网页行情端点。它们不是本项目可证明具有 SLA、
版本合同或再分发许可的正式商业 API，因此只能作为基础行情补充源，不能代替交易所
授权 Level-2、Wind、Choice 等生产主源。部署方必须自行确认服务条款、调用频率和
数据展示/再分发授权。

客户端不读取桌面客户端，不使用账号、Cookie、设备令牌，不代理、解密或重放登录
会话。每次请求都有连接、读取和写入超时，单个响应上限 1 MiB；没有模拟数据、陈旧
缓存或跨 Provider 静默回退。

## 已实现能力

| 统一契约 | 上海/深圳 | 北京 | 精确边界 |
| --- | --- | --- | --- |
| `RealtimeQuotes` | 已实盘通过 | 已实盘通过 | 名称、价量额、涨跌幅、源时间 |
| `OrderBooks` | 已实盘通过 | 已实盘通过 | 五档价格/数量与可见总深度 |
| `HistoricalBars` | 1/5/15/30/60 分钟、日/周/月 | 仅日线实盘通过 | 未复权；年线明确不支持 |
| `MinuteData` | 当日与按日期历史 | 当日及历史端点已验证 | 累计量；累计额缺失时保持 `None` |
| `Trades` | 仅当日、自动翻页 | 不支持 | 最多 2,000 条；源端无已验证日期选择器 |
| `SecurityMetadataProvider` | 部分 | 部分 | 名称/ST 来自快照，板块派生；上市日和涨跌停规则缺失 |
| `MarketStatisticsProvider` | 股票/指数/ETF 实盘 | 仅股票身份可请求 | 换手、PE/PB、市值、涨跌停价、量比；需完整 53 字段 |
| 财务、除权、板块 | 不支持 | 不支持 | 当前端点没有可审计合同 |
| 资金流、集合竞价 | 不支持 | 不支持 | 不从 Quote、盘口或逐笔推测 |

2026-07-23 的 release 探针实测了华电辽能 `600396.SH`、平安银行
`000001.SZ` 和太湖远大 `920118.BJ`。三只证券均返回真实 Quote 和五档；北京市场
响应编码为 `62`。华电辽能的 1/5/15/30/60 分钟、日/周/月 K 线、当日分时、指定
日期分时和 20 条当日逐笔均通过。太湖远大的 Quote、五档、日线和当日分时通过；
北京分钟 K 线和逐笔端点返回空，因此入口直接返回带原因的 `Unsupported`。

腾讯 `year` 参数在实盘返回“当前日线形状”的记录，并不是年线，适配器不会把它
冒充年线。K 线统一使用显式 `none` 的未复权数组；当前 Core 请求没有复权选择器，
因此不接受前/后复权语义。日期范围不能由端点原子满足时也不会被静默忽略。

## 端点与网络

| 数据族 | HTTPS 端点 |
| --- | --- |
| Quote、五档、部分元数据 | `qt.gtimg.cn` |
| 日/周/月 K 线、当前/历史分时 | `web.ifzq.gtimg.cn` |
| 分钟 K 线 | `ifzq.gtimg.cn` |
| 当日逐笔 | `stock.gtimg.cn` |

部署防火墙需要允许这些域名的 TCP 443。必须按域名解析，不能固定当前 IP；任何
DNS、TLS、HTTP、解码或字段矛盾都会返回错误，不会切换到 TDX。

## 字段、单位与证据

Quote 源响应是 GBK 编码、`~` 分隔的 JavaScript 赋值行。已验证的核心位置为：

| 位置 | 含义 | 统一输出 |
| ---: | --- | --- |
| 0/1/2 | 市场、名称、代码 | `InstrumentId` 与 `Quote.name` |
| 3/4/5 | 当前、昨收、今开 | CNY 价格 |
| 6 | 累计成交量 | 源端“手”，不乘 100 |
| 9..18 | 买一至买五价量 | CNY / 源端“手” |
| 19..28 | 卖一至卖五价量 | CNY / 源端“手” |
| 30 | `YYYYMMDDHHMMSS` | ISO 8601 中国市场时间（`+08:00`） |
| 32/33/34 | 涨跌幅、最高、最低 | 百分比 / CNY |
| 35 | `price/volume/amount` | 第三项按 CNY 元输出 |
| 38/39 | 换手率/动态 PE | 百分比 / 有限数；空值保持 `None` |
| 44/45 | 总市值/流通市值 | 源端亿元乘 `100,000,000` 输出 CNY 元 |
| 46 | PB | 有限数；源端显式零保留为零 |
| 47/48 | 涨停价/跌停价 | 正 CNY 价格；指数 `-1` 和源端 `0` 占位转 `None` |
| 49/52 | 量比/静态 PE | 有限数；空值保持 `None` |

`Quantity` 本身不携带单位，所以 Quote、盘口、K 线、分时和逐笔的数量均按源端
“手”解释；逐笔响应的成交额只用于校验 `price × lots × 100`，不伪造 Core 中没有
的字段。

记录的 `source_at` 只来自源报文；`observed_at` 是本机 Unix 时间。仅当所有记录
都有可靠源时间时，批次才携带 `source_at`。零价档位原子地映射为不可用；有数量
无价格、负数、非有限数、重复/遗漏证券、乱序矛盾、复合字段冲突、无效 GBK/JSON
和翻页序号间隙均失败。

历史分时实盘出现 267 条：正常 242 个分钟点之外，源端还给出 15:06 至 15:30 的
25 个盘后累计点。解析器只额外允许这个已验证窗口；15:01 至 15:05、15:31 以后或
累计量/额倒退仍会失败。历史响应没有成交额时，每点保持 `None` 并把批次标为部分
可用。

## 使用与验收

确定性测试不联网：

```bash
cargo test -p magic-tencent-rs --all-targets --locked --offline
```

真实探针默认查询三市场基础行情，并查询华电辽能、上证指数和 510050 ETF 的行情
统计，打印 Quote、五档、元数据、统计、各周期 K 线、当前/历史分时和当前逐笔的
全部标准化字段：

```bash
cargo run -p magic-tencent-rs --example live_probe --release --locked --offline
```

```text
MAGIC_TENCENT_CODES=600396.SH,000001.SZ,920118.BJ
MAGIC_TENCENT_STATISTICS_CODES=600396.SH:EQUITY,000001.SH:INDEX,510050.SH:ETF
MAGIC_TENCENT_HISTORY_DATE=2026-07-22
MAGIC_TENCENT_TIMEOUT_SECS=10
```

并发探针支持 `quotes`、`bars`、`minute`、`trades`、`statistics` 和轮转五类请求
的 `mixed`：

```bash
cargo run -p magic-tencent-rs --example load_probe --release --locked --offline
```

```text
MAGIC_TENCENT_LOAD_OPERATION=mixed
MAGIC_TENCENT_LOAD_REQUESTS=20
MAGIC_TENCENT_LOAD_CONCURRENCY=4
```

程序硬限制为最多 100 次请求、8 个工作线程，超限会在联网前失败。2026-07-23 的
真实 `mixed` 上限探针为 100 请求/8 并发，100/100 成功、3,700 条记录、56.49
req/s、P50 100.077 ms、P95 219.676 ms、最大 251.169 ms。这个短样本不是厂商
SLA，也不是可持续频率建议；生产调用必须有自己的限频、熔断和授权策略。

同日 `statistics` 专项探针为 12 请求/3 并发，12/12 成功、36 条记录、28.76
req/s、P50 66.801 ms、P95 181.955 ms、最大 192.500 ms。

盘前当前价为零时，统一 Quote 无法构造正价格，命令会显式失败；涨跌停、停牌等
导致盘口一侧缺失时，记录保留已有档位并标为 `Unavailable`。上层切源时必须保留
真实 `ProviderId`、`source_at`、`observed_at` 和 `batch_id`。
