# 同花顺公开研究与信号接入

`magic-ths-rs` 是只读的同花顺公开网页 Provider，覆盖一致预期、强势股原因、涨停池
和股票热榜。它不读取浏览器 Cookie、桌面客户端登录态、账号、选股条件或交易数据。

## 数据源与网络边界

只允许下列 HTTPS 443 主机：

```text
basic.10jqka.com.cn
zx.10jqka.com.cn
data.10jqka.com.cn
dq.10jqka.com.cn
```

对应端点包括个股 `worth.html` 一致预期页、强势股公开数据、涨停池
`dataapi/limit_up/limit_up_pool` 和热榜
`fuyao/hot_list_data/out/hot_list/v1/stock`。

客户端禁止重定向，默认超时 15 秒，单响应最多 4 MiB。所有克隆共享串行请求门，
完整响应读取期间并发为 1，请求起始间隔至少 1 秒。

## 标准化数据

| 数据族 | 入口 | 关键字段 |
| --- | --- | --- |
| 一致预期 | `ConsensusData` | 年份、EPS 最小值/均值/最大值、贡献机构数和源日期 |
| 强势原因 | `StrongStockReasons` | 证券、日期、原因、题材/概念和证据 |
| 涨停池 | `LimitPools` | 价格、涨幅、封单额、首封时间、开板次数、连板数、封板状态和源端原因 |
| 人气榜 | `PopularityData` | 排名、证券、名称、涨跌幅、热度、标签/概念 |

`limit_up_type` 映射为封板状态，`high_days` 只用于连板数；当前响应没有被验证为
行业、真实板块或末封时间来源，因此 `industry`、`board_name` 和 `last_seal_at`
保持 `None`。当前只声明涨停池及源端原因，不声称已实现炸板、跌停或昨日涨停池。

## 请求边界

- 一致预期每批最多 20 个证券；
- 强势股每次最多 200 条；
- 涨停池每次最多 200 条；整市场消费者必须请求 200 条 transport bound，并在
  Provider 校验源日期、第一页、源总数与完整唯一行数后才可自行筛选/限量；
- 热榜每次最多 100 条；
- strict 请求若没有目标证券或返回空榜，会返回 typed `Incomplete`；涨停池只有在
  exact-date 响应明确 `total=0` 且 `info=[]` 时返回带 provenance 的完整空批次；
  一致预期页明确写出“暂无机构做出业绩预测”时返回带请求身份、源日期、
  观测时间和批次号的 typed `VerifiedEmpty`，不会构造空 estimates 伪记录；
- HTML/JSON 结构、日期、URL、非有限数或数量上限不满足时显式失败。

## 探针

```bash
cargo run -p magic-ths-rs --example live_probe --release --locked --offline

MAGIC_THS_LOAD_REQUESTS=3 \
MAGIC_THS_LOAD_CONCURRENCY=1 \
MAGIC_THS_LOAD_PACING_MS=1000 \
cargo run -p magic-ths-rs --example load_probe --release --locked --offline
```

live probe 默认验证贵州茅台 `600519.SH` 一致预期、美利云 `000815.SZ` 强势原因、
指定交易日涨停池和当前热榜。每个非空 family 必须通过公共 admission verifier，
并打印 `family=<name> status=admitted`；源明确为空的一致预期打印
`status=verified_empty`。普通空批次、incomplete quality、issues、批次/记录证据不
一致、未来源时间或重复业务身份都会非零退出。最终成功标记固定为
`live_probe_status=admitted`。

load probe 当前只对热榜做最多五次的串行、有间隔诊断；结果不是端点 SLA 或允许
调用频率。其他 advertised family 的独立 load case 属于 Task 8 后续切片，在完成前
不能用 popularity-only 结果声称全能力 load admission。

## 生产边界

公开网页结构和字段可能变化，也没有本项目可证明的版本合同、SLA 或再分发许可。
这些数据只适合作为盘后研究和信号补充；调用方必须自行处理授权、缓存、调度、熔断
和持久化。本 crate 不提供 Cookie 绕过、隐藏重试、跨源补值或模拟数据。
