# QualifiedTradingFacts（op 67）provider-sourced 观察值合同

- 操作：`MarketDataService.QualifiedTradingFacts`，append-only 的第 67 个只读操作（`OPERATION_QUALIFIED_TRADING_FACTS = 67`）。
- 请求 schema：`magic.market.qualified_trading_facts.request`，版本 1。
- 记录 schema：`magic.market.qualified_trading_fact`，版本 1（单数，一行一个 (instrument, date)）。
- 能力 provider：`Composition`（组合级：内部一次 TDX `SecurityMetadata` +（按需）一次腾讯 `MarketStatistics`）。
- 状态：proto 已登记；部署后由 `GetCapabilities` 精确返回 scope。本文件随 Gate C 证据冻结。

## 1. 请求

```json
{
  "instruments": [
    {"exchange": "Shanghai", "code": "600519", "asset_class": "Equity"}
  ],
  "dates": ["2026-10-16"],
  "include": ["lifecycle", "price_band"]
}
```

- `instruments`：1..=50 个，去重，仅沪深京 A 股个股权益（`asset_class=Equity`）。
- `dates`：1..=250 个，去重，`YYYY-MM-DD`，且必须落在签入的 SSE 2026 交易日修订内；越界在 provider 调用前 typed `InvalidRequest`。
- 行数预算：`instruments × dates ≤ 500`。
- `include`：可省略；省略等价于 `["lifecycle","price_band"]`。显式空数组无效。可选值 `lifecycle`、`price_band`、`suspension`；**只要包含 `suspension` 即整单 typed `Unsupported`**（reason 原文 `no admitted suspension source exists for qualified trading facts`）。
- 位置参数与字段名均为精确匹配，拒绝未知字段。

## 2. 记录

```json
{
  "instrument": {"exchange": "Shanghai", "code": "600519", "asset_class": "Equity"},
  "date": "2026-10-16",
  "quality": "provider_observed",
  "source": "tdx-security-metadata",
  "observed_at": "2026-10-17T10:21:25+08:00",
  "source_at": null,
  "listed_on": "2001-08-27",
  "listing_state": "listed",
  "limit_up": 1444.27,
  "limit_down": 1181.67,
  "price_band_source": "tencent-market-statistics",
  "evidence": {
    "provider": "Tdx",
    "source_at": null,
    "observed_at": "1792203685",
    "batch_id": "tdx:1792203685:security-metadata"
  },
  "price_band_evidence": {
    "provider": "Tencent",
    "source_at": "2026-10-16T16:14:47+08:00",
    "observed_at": "1792203713.191533300",
    "batch_id": "tencent-web:1792203713.191533300:market-statistics"
  }
}
```

字段语义：

- `quality`：v1 恒为 `provider_observed`；`authoritative` 永不产出。
- `source`：该行实际承载子集的来源。含 `lifecycle` 时为 `tdx-security-metadata`；仅 `price_band` 时为 `tencent-market-statistics`。
- `observed_at`：本侧观测时刻，RFC3339、时区 `+08:00`、毫秒精度（无小数则为秒）；由内部批次 epoch 值转换。`evidence` 内保留 provider 原始 `observed_at` 字符串。
- `source_at`：`lifecycle` 行恒为 `null`（TDX 元数据无源时刻，不代填）；`price_band`-only 行携带腾讯统计批次的 `source_at`。
- `listed_on`：源值原样（`YYYY-MM-DD`）；无来源为 `null`。
- `listing_state`：`listed`（`listed_on ≤ date`）/ `not_yet_listed`（`listed_on > date`）/ `unknown`（`listed_on` 缺失）。**v1 永不产出 `delisted`**（无退市来源）。`listed` 不证明"在交易"（无停牌来源）。
- `limit_up` / `limit_down`：腾讯 `MarketStatistics` 记录的源值（`upper_limit`/`lower_limit`），仅在**观测快照日**有值；其余日期为 `null`。
- `price_band_source`：有值即 `tencent-market-statistics`，且仅当该行确有至少一个涨跌停源值；`null` 表示该日价格带不可用。
- `price_band_evidence`：价格带批次的 `SourceEvidence`（透传腾讯统计记录自身 evidence）；无价格带为 `null`。
- `evidence`：lifecycle 行的 `SourceEvidence`（provider=`Tdx`、`source_at=null`、原始 `observed_at`、`batch_id`）。

**观测快照日**的定义：腾讯统计批次 `source_at` 的 Asia/Shanghai 自然日（不是服务器当天）；节假日查询自动落到最近交易日。请求 `dates` 不含该日时，全部行的价格带字段为 `null`，批仍为完整。

## 3. 排序与完整性

- 行序：`instruments` 输入顺序 × `dates` 升序；同一请求除 `observed_at`/`batch_id` 与行情值外逐字段稳定。
- `complete=true` 表示：全部请求行均已按上述边界产出；**不**表示停牌状态、历史价格带或退市状态完整。
- 任一内部批次 typed 失败、或批次缺少请求内任一标的 → 整个调用 typed 失败（fail-closed），不产出部分行。

## 4. 错误

- `include` 含 `suspension`：gRPC `UNIMPLEMENTED`，`ErrorDetail{reason_code:"capability_unadmitted", retryable:false, admission:Unadmitted}`（二进制 metadata 键 `magic-error-detail-bin`）。
- 越界 instruments/dates/行数、日历外日期、非权益标的、重复项、未知字段：`INVALID_ARGUMENT`。
- provider 批次缺标的/缺 `source_at`/无可用批次：`FAILED_PRECONDITION`。
- 上游 TDX / 腾讯故障按既有 provider 错误映射（可重试与不可重试语义不变）。

## 5. 边界（明确不做）

- 不交付 `suspension`（任何形态；包括"空数组=无停牌"）；
- 不交付历史日期的价格带，不做任何价格带推导；
- 不产出 `delisted`、不推断退市；
- 不存储、不缓存、不建索引；
- 不改变 `SecurityMetadata` / `MarketStatistics` 既有记录合同。

## 6. 与 Mac P0-1 规格的关系

对应 `grpc_handoffs/2026-10-17-p01-provider-sourced-trading-facts-spec.md` 的 B 路线（provider-sourced 观察值）：`lifecycle` 与观测日 `price_band` 可交付；`suspension` 明确不交付。设计依据与决策记录见
[`2026-10-17-qualified-trading-facts-gate-a-design.md`](2026-10-17-qualified-trading-facts-gate-a-design.md)，业务规则见 `BR-071`。
