# 百度技术 K 线接入

`magic-baidu-rs` 是只读的日线技术指标适配器，使用百度股市通公开 HTTPS
接口，并把源端已经计算的 MA5/MA10/MA20 映射为 Core `TechnicalBar`。
`TECHNICAL_BARS_ADMITTED=true` 只准入这个独立合同；通用
`HistoricalBars` 仍保持 `capabilities().bars=false`。

## 数据源与网络边界

唯一允许的请求目标是：

```text
https://finance.pae.baidu.com/selfselect/getstockquotation
```

传输层只允许 `finance.pae.baidu.com:443`、禁止跳转，只接受 HTTP 200 JSON，
单响应最多 8 MiB。所有客户端 clone 共享一个串行门：生产请求开始时间至少相隔
1 秒，且门会持有到完整响应读取结束。一次请求最多返回 2,001 根，只支持一个明确
证券的日线；分钟、周/月线、日期选择器和多请求拼接均明确拒绝。

证券代码会在发送前校验市场归属：`6` 开头只能是上海，`0`/`3` 开头只能是深圳，
`4`/`8`/`920` 开头只能是北京；未验证的其他 `9` 字头（例如上海 B 股
`900901`）会被拒绝。市场和代码不匹配不会发送请求。

## 单位与调整语义

- OHLC 使用 Core 正价格；
- 源成交量为股，在 Provider 边界除以 100 标准化成手；
- 成交额保留为人民币；
- 该请求没有复权选择参数；真实除权日前后的价格缺口仍然存在，因此固定标为
  `Adjustment::Unadjusted`，不把原始价格误称为前复权；
- 源 `--` 的 MA 值保留为 `None`，不补 0、不在本地重算；
- 每根 Bar 与外层 `TechnicalBar` 都保留 Baidu provider 和同一批次证据。

## 探针

```bash
cargo run -p magic-baidu-rs --example live_probe --release --locked --offline
MAGIC_BAIDU_LOAD_REQUESTS=2 \
  cargo run -p magic-baidu-rs --example load_probe --release --locked --offline
```

默认样本为华电辽能 `600396.SH`。live probe 只请求一根记录并用公共证据门校验
OHLC、量额、未复权标记、MA5/10/20 和完整证据。它验证的是源端逐日事实，
不宣称复权后的相邻价格连续性。load probe 串行、由客户
端保证请求起始至少间隔一秒、最多三次，并输出
成功/失败、错误、RPS 和 p50/p95/p99/max。确定性 fixtures 已覆盖字段映射、缺失
MA、真实除权日缺口、市场/代码一致性、畸形 Content-Type、clone 并发门、行数上限
和 URL 白名单。2026-07-23 的历史真实 probe 曾返回华电辽能 5 根未复权日线及
MA5/10/20，但该记录早于当前机器准入门，不构成 capability admission；历史负载
结果详见 `docs/PERFORMANCE_RESULTS.md`。

2026-08-16 的独立 release 诊断成功返回华电辽能 2026-08-14 一根完整日线：
OHLC `18.18/18.20/16.91/16.99`、成交量 `2,835,626.06` 手、成交额
`4,962,294,940` 元、MA5/10/20 `17.38/17.14/16.05`。

2026-08-17 两次独立 release live probe 均返回华电辽能当日完整未复权日线：
OHLC `16.44/17.28/16.43/16.92`、成交量 `2,033,241` 手、成交额
`3,423,287,884` 元、MA5/10/20 `17.32/17.28/16.25`。随后三次串行
load 均成功，共返回 60 条，最小请求起始间隔不低于 1 秒且最大并发为 1。
该精确 `TechnicalBars` 合同因此生产准入。

## 生产边界

该接口不提供实时 Quote、分钟线或 Level-2，也没有可证明的 SLA。正式结果只表示
源端提供的未复权逐日 OHLCV/amount 与可选 MA 值，不表示交易日历完整、复权连续、
公司行动已解释或可以替代独立的 `HistoricalBars` Provider。
