# Sina 公共网页行情接入合同

## 定位

`magic-sina-rs` 是只读补充 Provider。它访问新浪公开行情域名，不读取桌面或移动
客户端，不使用 Cookie、账号、验证码、设备令牌、持仓、资金或下单接口。公开网页
接口没有本项目可证明的版本合同、SLA、展示许可或再分发许可，生产部署方必须自行
确认调用频率与使用条款。

2026-07-23 的真实探针覆盖：

- 华电辽能 `600396.SH`；
- 平安银行 `000001.SZ`；
- 太湖远大 `920118.BJ`；
- 华电辽能的资产负债表、利润表和现金流量表；
- 上证 50 ETF `510050` 的合约月份、全部认购/认沽合约、最优买卖一档 T 型报价和
  希腊字母。

一次实测只证明当时端点和字段可用，不构成厂商性能承诺。字段或协议变化后必须重新
运行确定性测试和真实探针。

## 公开端点

| 数据族 | HTTPS 端点 | 请求边界 |
| --- | --- | --- |
| Quote / 五档 / 部分元数据 | `https://hq.sinajs.cn/list=` | 最多 50 个不重复沪深京 A 股符号；必须发送 `Referer: https://finance.sina.com.cn/` |
| K 线 / 当前分时输入 | `https://quotes.sina.cn/cn/api/json_v2.php/CN_MarketDataService.getKLineData` | 单证券；最多 800 根 K 线；当前分时固定最多 300 根 1 分钟线 |
| 财务三表 | `https://quotes.sina.cn/cn/api/openapi.php/CompanyFinanceService.getFinanceReport2022` | 最多 10 个沪深证券；每类默认 8 个报告期 |
| ETF 期权月份 | `https://stock.finance.sina.com.cn/futures/api/openapi.php/StockOptionService.getStockName` | 单个受支持标的；最多 12 个月 |
| ETF 期权合约、T 型报价、希腊字母 | `https://hq.sinajs.cn/list=` | 合约发现最多 4,096 个；单批最多 50 个；必须发送 `Referer: https://stock.finance.sina.com.cn/` |
| 全球指数 / 外汇 | `https://hq.sinajs.cn/list=` | 精确请求、精确返回；6 个已验证指数与 8 个已验证汇率对 |

客户端只接受 HTTPS，拒绝重定向，connect/read/write 使用正的有界超时，单响应最多
1 MiB。克隆 `SinaClient` 会共享同一个 `ureq` 连接池。

## 全球指数与汇率

`GlobalIndexProvider` 支持 Dow Jones、NASDAQ Composite、S&P 500、Nikkei 225、
Hang Seng 和 FTSE 100；`ForeignExchangeProvider` 支持 USD/CNY、EUR/USD、
USD/JPY、GBP/USD、AUD/USD、USD/CHF、USD/CAD、NZD/USD。请求必须非空、去重且不
超过 20 个身份，响应必须与请求符号及数量精确一致。所有值必须有限，涨跌幅保留
百分比单位。外汇包中的源日期/时间进入证据；全球指数包没有可靠源时间时明确保留
`source_at=None`。

## Quote 编码与字段

响应按 GB18030 严格解码，每条必须匹配：

```text
var hq_str_<sh|sz|bj><六位代码>="<逗号字段>";
```

沪深京共同且已使用的字段为：

| 位置 | 含义 | 标准化 |
| --- | --- | --- |
| 0 | 名称 | `Quote.name`、ST 名称证据 |
| 1 | 开盘 | CNY `Price`；0 保持缺失 |
| 2 | 昨收 | CNY `Price`；0 保持缺失 |
| 3 | 现价 | 必须为有限正数 |
| 4 / 5 | 最高 / 最低 | CNY `Price`，校验 OHLC 范围 |
| 6 / 7 | 最佳买 / 卖 | 与一档价格交叉校验 |
| 8 | 累计成交量 | 源端“股”除以 100，输出“手” |
| 9 | 累计成交额 | CNY 元 |
| 10/11..18/19 | 买一至买五量/价 | 股→手 / CNY |
| 20/21..28/29 | 卖一至卖五量/价 | 股→手 / CNY |
| 30 / 31 | 源日期 / 源时间 | `YYYY-MM-DDTHH:MM:SS+08:00` |
| 32 | 市场状态 | 必须存在；不推导业务状态 |

北京响应存在不同的尾部字段。本 Provider 只要求并使用共同字段 0..32，不把未验证的
尾部字段升级为能力。

`Quantity` 本身没有单位标签。为和现有 TDX/Tencent 标准化调用保持一致，所有 Sina
源端“股”数量都在适配边界除以 100：

- Quote 累计量；
- 买卖五档数量；
- K 线成交量；
- 分时累计量。

不得对某个数据族漏做换算。成交额始终按 CNY 元输出。

## 批次与错误

多证券快照保持请求顺序和精确基数。以下情况返回错误，不返回部分成功：

- 空请求、超过 50 个代码、重复代码或非沪深京六位 A 股；
- 空响应、非法 GB18030、字段不足或符号键矛盾；
- 响应缺行、重复行或包含未请求代码；
- 非有限/负量额、非正现价、非法日历时间或 OHLC 矛盾；
- 冗余最佳价与一档价格矛盾；
- 网络、TLS、HTTP 或响应上限错误。

涨跌停时盘口一侧可能真实为 `0/0`。这种档位输出
`BookLevel::unavailable()`，OrderBook 标记 `Unavailable` 并附质量问题；不会伪造
五档。价格和数量只有一项存在也标质量问题或协议错误。

每条记录保留 `ProviderId::Sina`、`source_at`、`observed_at` 和 `batch_id`。
provenance source 固定为 `sina-web`；只有批次中每条快照都有源时间时，批次才写
`source_at`。

## K 线

支持范围：

| Core 周期 | Sina scale | 沪/深 | 北京 | 成交额 |
| --- | ---: | --- | --- | --- |
| `Minute1` | 1 | 已实测 | 端点支持；live probe 主要验上海 | 必须存在 |
| `Minute5` | 5 | 已实测 | 已实测 | 必须存在 |
| `Minute15` | 15 | 已实测 | 代码支持 | 必须存在 |
| `Minute30` | 30 | 已实测 | 代码支持 | 必须存在 |
| `Hour1` | 60 | 已实测 | 代码支持 | 必须存在 |
| `Day` | 240 | 已实测 | 已实测 | 官方响应缺失，保持 `None` |

`Week`、`Month`、`Year` 和 `BarsRequest::with_range` 明确
`Unsupported`。所有返回标记为 `Adjustment::Unadjusted`，因为当前合同没有验证
复权选择参数。

JSON 根必须为非空数组。每行必须含字符串
`day/open/high/low/close/volume`；分钟行还必须含字符串 `amount`。解析器拒绝数量
超过请求、重复、倒序、非法时间、非法数值和 OHLC 矛盾。分钟记录的源时间转换为
UTC+8 ISO 格式；日线源证据是日期。

## 当前分时

新浪已探测到的专用 minline 服务名返回 `Service not found`，因此不宣称专用分时
API。`MinuteData` 无日期请求使用严格受限的 300 根 `scale=1` K 线窗口：

1. 校验全部 1 分钟行；
2. 选择响应中最新交易日；
3. 按源时间递增；
4. 累加该日每分钟成交股数和 CNY 成交额；
5. 分钟收盘价作为 `MinutePoint.price`；
6. 累计股数除以 100 输出“手”。

带日期的历史分时请求在联网前返回 `Unsupported`。周末或休市时“最新交易日”可能
不是当天；调用方必须检查 `source_at`，不能把最新可得记录冒充实时数据。

Quote 与 K 线来自不同公开端点，不是一个原子快照，累计量额可能有小幅差异。Provider
不跨端点改写或拼接记录；调用方可以用各自的源时间和观测时间进行冲突/新鲜度判断。

## 财务三表

`FinancialStatements` 使用新浪公开 `CompanyFinanceService`，分别请求：

| Core 报表 | 新浪 source |
| --- | --- |
| 资产负债表 | `fzb` |
| 利润表 | `lrb` |
| 现金流量表 | `llb` |

响应根必须包含 `result.data.report_list`。每个报告期保留来源报告日、发布日期、币种、
原始英文 `item_field`（标准化为小写）、中文标题和可空数值；不猜测或补造单位。结构性
空标题会被跳过。来源有时会返回完全相同的重复字段，Provider 会保留一份并在记录质量
中注明；同名但值或标题冲突的字段会直接报协议错误。

当前只支持上海、深圳证券；北京证券在联网前返回 `Unsupported`。每类报表默认返回
最近 8 个报告期，批量请求最多 10 个不重复证券。真实探针逐一验证三张报表非空、报告期
有序、字段键唯一且来源证据完整。

## ETF 期权

`OptionData` 已实现新浪公开期权页覆盖的四个上海 ETF 标的：

- `510050` 上证 50 ETF；
- `510300` 沪深 300 ETF；
- `588000` 科创 50 ETF；
- `510500` 中证 500 ETF。

2026-07-23 的真实网络验收只覆盖 `510050`。`510300`、`588000` 和 `510500`
保留为已实现待实测，分别通过完整 live probe 前不得标为实盘可用。

Provider 先发现有效 `YYYY-MM` 合约月份，再分别读取认购/认沽列表并生成稳定
`OptionContract`。T 型报价保留名称、标的、方向、月份、最新价、涨跌、开高低、昨结、
成交量、成交额、持仓量、最优买卖一档、涨跌停以及严格标准化的来源时间。源响应中
更深的槽位缺少本项目可验证的稳定字段合同，因此不标准化、也不宣称五档期权盘口。
希腊字母保留 Delta、Gamma、Theta、Vega、隐含波动率和来源可得字段；来源没有 Rho
时保持 `None`。

解析器拒绝非法月份、未知标的、重复/矛盾合约、负成交量额、交叉盘口、非法价格范围、
字段不足或超上限。边界为 12 个合约月、每个认购/认沽列表 256 个合约、总发现
4,096 个、单批报价或希腊字母 50 个。响应壳最多 257 个字段；报价行最多 64 个，
希腊字母行最多 32 个。不会用 Black-Scholes 自行计算并冒充来源希腊字母或 IV。

## 部分证券元数据

来源快照提供名称，ST 标志只按名称前缀识别。板块按交易所和代码派生：

- 北京交易所 → `Board::Beijing`；
- 上海 `688` → `Board::Star`；
- 深圳 `300`/`301` → `Board::ChiNext`；
- 其他沪深 → `Board::Main`。

快照没有上市日期、来源涨跌停规则和规则版本，所以记录状态与批次质量始终明确为
部分不可用。代码派生板块不能冒充来源证券主数据。

## 能力声明

```text
quotes=true
bars=true
minute=true          # 仅最新交易日，无历史日期选择
order_book=true
security_metadata=true
trades=false
fundamentals=true       # 资产负债表、利润表、现金流量表
corporate_actions=false
blocks=false
money_flow=false
auction=false
```

期权能力由独立的 `OptionCapabilities` 声明：

```text
contract_discovery=true
quotes=true
greeks=true
```

成交明细展示 HTML 不作为稳定逐笔合同。不得从 Quote、五档或 K 线推导并冒充
MoneyFlow、集合竞价、公司行为或板块来源字段。

## 运行

全功能真实探针：

```bash
MAGIC_SINA_CODES=600396.SH,000001.SZ,920118.BJ \
MAGIC_SINA_OPTION_UNDERLYING=510050 \
MAGIC_SINA_OPTION_SAMPLE_CONTRACTS=2 \
MAGIC_SINA_TIMEOUT_SECS=10 \
cargo run -p magic-sina-rs --example live_probe --release --locked --offline
```

探针打印 6 个全球指数、8 个汇率对、Quote、全部五档、部分元数据、六个支持周期、
北京 5 分钟/日线、每个证券的当前分时点、三张财务报表的全部报告期、全部发现的
期权合约、样本 T 型报价/希腊字母和所有不支持能力。任一预期数据族为空、数量不符
或协议错误会退出非零。

有界并发探针：

```bash
MAGIC_SINA_LOAD_OPERATION=mixed \
MAGIC_SINA_LOAD_REQUESTS=20 \
MAGIC_SINA_LOAD_CONCURRENCY=4 \
cargo run -p magic-sina-rs --example load_probe --release --locked --offline
```

operation 可选 `quotes`、`bars`、`minute`、`financial`、`options`、`mixed`。
`options` 默认在计时和启动 worker 前，按 `MAGIC_SINA_OPTION_UNDERLYING`（默认
`510050`）执行一次当前合约发现，并选择
`MAGIC_SINA_OPTION_SAMPLE_CONTRACTS`（默认 2）个合约。部署方也可用
`MAGIC_SINA_OPTION_CONTRACTS` 显式覆盖；没有固定的到期合约后备值。程序在联网前
强制最多 40 请求、4 线程。2026-07-23 的默认基础数据混合样本结果：

```text
requests=20 concurrency=4 successes=20 failures=0 records=1477
requests_per_second=11.69
latency_us_p50=207786 latency_us_p95=645489 latency_us_max=788549
```

专项真实短样本：

```text
operation=financial requests=6 concurrency=2 successes=6 failures=0 records=48
requests_per_second=18.19
latency_us_p50=50705 latency_us_p95=210571 latency_us_max=213895

operation=options requests=6 concurrency=2 successes=6 failures=0 records=24
requests_per_second=22.30
latency_us_p50=62005 latency_us_p95=131468 latency_us_max=144344
```

这只是一次有界短样本，不是厂商 SLA 或推荐限频。

## Router 注册与生产责任

Router 的生产依赖保持只有 Core。应用在自己的组合根使用现有
`quote_source`、`bars_source`、`minute_source`、`order_book_source` 和
`security_metadata_source` 注册基础 Sina Provider；财务三表和 ETF 期权也通过 Core
对应的来源适配器注册，不要求 Router 依赖 Sina crate。

生产服务必须额外实现 Provider 级限频、退避、熔断、交易阶段新鲜度门、监控和合法
缓存。Sina client 应长期复用，不能为每条记录启动 probe。防火墙只需允许：

```text
hq.sinajs.cn:443
quotes.sina.cn:443
stock.finance.sina.com.cn:443
```

Provider 不写持久缓存，不需要用户名、密码、Cookie 或本地客户端。部署与日志不得
增加这些敏感数据。
