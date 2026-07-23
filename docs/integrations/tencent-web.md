# 腾讯网页行情补充源

## 定位与边界

`magic-tencent-rs` 只读访问 `https://qt.gtimg.cn/q=`。这是一个可直接
联通的网页行情端点，不是本项目可证明有 SLA、版本合同或再分发许可的正式商业
API，因此只能作为基础行情补充源，不能代替交易所授权 Level-2、Wind、Choice 等
生产主源。部署方必须自行确认服务条款、使用频率和数据展示/再分发授权。

客户端不读取东方财富或腾讯桌面客户端，不使用账号、Cookie、设备令牌，不代理、
解密或重放任何登录会话。每次请求最多 50 只证券，响应上限 1 MiB，连接、读取和
写入均有超时；没有模拟数据和静默回退。

## 已实现能力

| 统一契约 | 状态 | 证据 |
| --- | --- | --- |
| 沪深 A 股实时 Quote | 已实现、实盘通过 | 名称、价量额、涨跌幅、源时间 |
| 五档盘口 | 已实现、实盘通过 | 买卖 1 至 5 档价格与数量 |
| K 线/分钟线 | `false` | 本端点未建立稳定的历史合同 |
| 逐笔、财务、除权、板块 | `false` | 未从该快照端点证明 |
| 资金流、集合竞价 | `false` | 不从 Quote/盘口推测或拼接 |
| 北京证券交易所 | 显式 `Unsupported` | 市场编码和单位尚未验证 |
| 指数、基金、债券 | 显式 `Unsupported` | 单位语义尚未验证 |

2026-07-23 的真实探针同时返回华电辽能 `600396.SH` 与平安银行
`000001.SZ` 的完整 Quote 和五档盘口，两个批次均为 `quality.complete=true`，
并携带逐记录源时间、采集时间、Provider 和批次 ID。

## 已验证字段与单位

源响应是 GBK 编码、`~` 分隔的 JavaScript 赋值行。适配器只读取下列已通过
fixture 与实盘交叉检查的位置：

| 位置 | 含义 | 统一输出 |
| ---: | --- | --- |
| 0/1/2 | 市场、名称、代码 | `InstrumentId` 与 `Quote.name` |
| 3/4/5 | 当前、昨收、今开 | CNY 价格 |
| 6 | 累计成交量 | 源端“手”，不乘 100 |
| 9..18 | 买一至买五价量 | CNY / 源端“手” |
| 19..28 | 卖一至卖五价量 | CNY / 源端“手” |
| 30 | `YYYYMMDDHHMMSS` | ISO 8601 中国市场时间 `source_at`（`+08:00`） |
| 32/33/34 | 涨跌幅、最高、最低 | 百分比 / CNY |
| 35 | `price/volume/amount` | 第三项按 CNY 元输出 |

`magic-market-core::Quantity` 本身不携带单位，所以使用腾讯记录时必须把 Quote
成交量和盘口数量解释为源端“手”；探针字段名也明确打印为 `volume_lots`、
`bid_lots` 和 `ask_lots`。成交额保留字段 35 的整数元值，不使用另一个以万元
显示的近似字段。

源时间经过数字、闰年、月日和时分秒边界校验。记录的 `source_at` 是来源报文时间；
`observed_at` 是本机 Unix 时间，两者不会混用。仅当每条记录都有源时间时，批次
`source_at` 才取本批最早记录，让新鲜度门使用保守时间；任一记录缺失时批次源时间
也保持空。零价档位原子地映射为不可用；“有数量但无价格”会进入质量问题。负数、
非有限数、重复/遗漏/多余证券、响应乱序矛盾、复合价量与独立字段冲突和无效 GBK
均失败。

## 使用与验收

确定性测试不联网：

```bash
cargo test -p magic-tencent-rs --all-targets --locked --offline
```

真实行情探针默认查询华电辽能和平安银行，并打印 Quote 所有字段、五档价量、
总可见深度、质量和证据：

```bash
cargo run -p magic-tencent-rs --example live_probe --release --locked --offline
```

可选设置：

```text
MAGIC_TENCENT_CODES=600396.SH,000001.SZ
MAGIC_TENCENT_TIMEOUT_SECS=10
```

短时并发探针默认 20 次请求、4 个工作线程：

```bash
cargo run -p magic-tencent-rs --example load_probe --release --locked --offline
```

```text
MAGIC_TENCENT_LOAD_REQUESTS=20
MAGIC_TENCENT_LOAD_CONCURRENCY=4
```

为避免配置错误压垮本机或无 SLA 的公网端点，程序硬限制不超过 100 次请求和 8 个
并发工作线程；超限在联网前失败。

盘前当前价可能为零，此时统一 Quote 无法构造正价格，命令会显式失败；涨跌停、停牌
等导致盘口一侧缺失时，盘口记录会保留已有档位并标为 `Unavailable`，不能把缺失档
伪装为零价格。公网端点失败也只返回错误，不回退到 TDX 或缓存；跨源切换应由上层
策略完成并记录 Provider。
