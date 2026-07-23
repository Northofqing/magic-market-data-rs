# Sina 公共行情 Provider 设计

状态：按用户“不用确认、全部做完”的持续授权采用本设计。

## 1. 目标与边界

新增独立的 `magic-sina-rs`，把新浪公开市场数据接到现有
`magic-market-core` 契约。首期交付覆盖已经在新浪官方域名真实验证的沪深京 A 股：

- 实时 Quote；
- 五档 OrderBook；
- 1/5/15/30/60 分钟 K 线和日线；
- 由完整 1 分钟 K 线窗口累计得到的最新交易日分时；
- 名称、ST 标志和代码派生板块组成的部分证券元数据。

本项目不读取新浪或其他客户端的登录态、Cookie、账户、持仓、资金和下单数据，
也不代理本地客户端流量。网页端点没有 SLA，Sina 定位为补充/故障切换来源，不宣称
交易所授权 Level-2。

## 2. 方案选择

采用“官方公开 Quote + K 线端点”：

- `https://hq.sinajs.cn/list=` 提供快照和五档；
- `https://quotes.sina.cn/cn/api/json_v2.php/` 下的
  `CN_MarketDataService.getKLineData` 提供 K 线；
- 当前分时复用 `scale=1` 的有界跨日窗口，只选最新交易日并累加源端每分钟成交量和
  成交额。

拒绝两个备选方案：

1. 只接 Quote。虽然简单，但不能满足项目 P0 的分钟线和日线需求。
2. 解析成交明细展示 HTML 或代理登录客户端。HTML 是展示合同而非稳定数据合同，
   客户端代理又引入版本、授权和敏感登录态风险。

## 3. 能力真值

`SinaClient::capabilities()` 返回：

| 能力 | 值 | 依据 |
| --- | --- | --- |
| `quotes` | `true` | 沪深京快照真实响应与严格夹具 |
| `bars` | `true` | 1/5/15/30/60/240 scale 真实响应 |
| `minute` | `true` | 最新交易日 1 分钟窗口可严格累计 |
| `order_book` | `true` | 快照字段 10..29 的买卖五档 |
| `security_metadata` | `true` | 名称/ST 有来源，板块按代码派生并标部分不可用 |
| 其他能力 | `false` | 没有完整、稳定、可审计的公开字段合同 |

`minute=true` 只代表无日期的最新交易日请求。带日期的历史分时明确返回
`Unsupported`。K 线只支持 `Minute1`、`Minute5`、`Minute15`、
`Minute30`、`Hour1` 和 `Day`；周、月、年及任意日期范围明确
`Unsupported`。

逐笔成交、MoneyFlow、Auction、财务、公司行为和板块不由本 Provider 实现。
不得从五档、成交额或价格变化推导这些来源字段。

## 4. 数量、金额与时间

新浪 Quote/K 线的成交量和五档数量是“股”，而现有跨 Provider 使用的
`Quantity` 数量约定是“手”。所有 Sina 数量在适配边界统一除以 100：

- Quote 累计成交量：股 → 手；
- 五档数量：股 → 手；
- K 线成交量：股 → 手；
- 分时累计量：逐分钟股数累加后 → 手。

成交额保持 CNY 元。分钟 K 线必须有 `amount`；日线官方响应没有成交额时保留
`None`，不得补零。

Quote 的源日期与源时间合并为
`YYYY-MM-DDTHH:MM:SS+08:00`。分钟 K 线使用行内时间；日线使用源日期。
本地 `observed_at` 与源时间分开，批次 `source_at` 只有在全部快照都有源时间时
才写入。

## 5. 快照解析

快照请求最多 50 个不重复的沪深京六位 A 股代码，并保持请求顺序和精确基数。
HTTP 请求必须：

- 使用 HTTPS；
- 设置新浪 Finance `Referer`；
- 设置明确 User-Agent；
- 使用正的 connect/read/write timeout；
- 拒绝重定向；
- 把响应限制在 1 MiB。

响应按 GB18030 严格解码。每条记录必须匹配
`var hq_str_<symbol>="...";`，至少包含已经验证的公共字段 0..32。符号键必须和请求
完全一致；重复、缺行、多行、空载荷和非法编码都返回协议错误。

Quote 校验有限正价格、非负量额、OHLC 范围和由现价/昨收计算的涨跌幅。五档价量必须
同时出现；`0/0` 表示该档不可用，只有价格或只有数量会产生质量问题。涨跌停导致一侧
为空时返回真实部分盘口和 `Unavailable`，不伪造完整盘口。

证券名称直接来自快照。ST 只按名称前缀识别，板块只按交易所/代码显式派生。上市日、
来源涨跌停规则和规则版本缺失，因此 `SecurityMetadata` 始终为部分不可用并列出问题。

## 6. K 线和分时解析

K 线请求最多 800 根，响应限制沿用 1 MiB。JSON 根必须是非空数组；每行要求：

- `day/open/high/low/close/volume` 全部存在且类型为字符串；
- 分钟行必须有非负 `amount`；
- 时间格式符合请求周期；
- 记录严格递增且不重复；
- 返回数不超过请求 `datalen`；
- Core `Bar` 再校验 OHLC。

所有 K 线标记为 `Adjustment::Unadjusted`，因为公开端点没有本设计已经验证的复权选择
合同。

当前分时固定请求最多 300 根 1 分钟 K 线，选择响应中的最新日期。逐行累加成交量和
成交额，输出 `MinutePoint.close` 作为分钟价，并校验时间递增、累计量额不回退。
空数组、最新日期无行或任何缺失金额都返回错误。返回的最新日期可能是上一交易日，
调用方必须继续使用 `source_at` 新鲜度门，不得把“最新可得”误当“当前实时”。

## 7. 代码结构

- `crates/magic-sina-rs/src/lib.rs`：客户端、HTTP transport、符号/快照解析、
  Quote、OrderBook 和部分元数据；
- `crates/magic-sina-rs/src/bars.rs`：K 线端点、周期映射和严格解析；
- `crates/magic-sina-rs/src/minute.rs`：最新交易日分时累计；
- `crates/magic-sina-rs/tests/capabilities.rs`：能力位与 trait 一致性；
- `crates/magic-sina-rs/examples/live_probe.rs`：打印每个已支持数据族和证据；
- `crates/magic-sina-rs/examples/load_probe.rs`：有界 quotes/bars/minute/mixed 并发；
- `docs/integrations/sina-web.md`：端点、字段、单位、边界、部署与命令。

Router 保持 Provider 中立，不增加 Sina 生产依赖。应用可直接使用现有
`quote_source`、`bars_source`、`minute_source` 和 `order_book_source` 注册
`ProviderId::Sina`。

## 8. 测试、并发与发布

确定性测试覆盖 GB18030、沪深京、请求排序、缺行/重复/多行、字段数、非法时间、
价量矛盾、涨跌停空侧、OHLC、K 线周期/顺序/数量、股到手换算、分时累计和所有
Unsupported 边界。

真实 `live_probe` 默认使用华电辽能 `600396.SH`、平安银行 `000001.SZ` 和太湖远大
`920118.BJ`，打印 Quote、五档、元数据、全部支持 K 线和当前分时。

负载探针硬限制 40 个请求和 4 个线程，默认 20/4，输出成功/失败、记录数、吞吐及
p50/p95/max 延迟，不包含自动重试。发布包新增 Sina live/load 两个探针，继续使用
干净隔离 target 和 SHA-256 清单。

最终通过 Rust 1.83 workspace check/test、strict Clippy、rustdoc/doctest、文档链接、
合规、真实 probe、有界并发和发布包校验后，才允许把能力写入 README。
