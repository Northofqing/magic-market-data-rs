# TradingCalendar：SSE 2026 签入修订合同（v1）

`TradingCalendar`（`MarketDataService.TradingCalendar`，第 66 个只读操作）返回上交所
（SSE）按年封存的交易日向量。它是**依官方公告、经复核的派生修订**：交易所不发布机器
可读的交易日清单，交易日 = 工作日 − 已公告休市日。运行时只读取仓库内审阅过的表，
不访问网络、不按公式外推、不改写任何其他 operation 的日期标注。

## 1. 请求

```json
{"year": 2026, "venue": "Sse"}
```

- schema：`magic.market.trading_calendar.request`，版本 1；
- `year` 为 2000..=9999 的正整数（Core `TradingDayRequest` 校验）；
- `venue` 为 `"Sse"` 或 `"Szse"`；当前仅 `Sse` 准入，`Szse` 在 Provider 边界返回
  typed `Unsupported`（"Shenzhen applicability is not established"）——实务同历是假设，
  未在本合同成立；
- 非 2026 年份在任何工作之前返回 typed `Unsupported`（"admitted only for 2026"）。
  新年度必须新增经审核的版本化表与测试，运行时永不扩展。

## 2. 记录（`magic.market.trading_day` v1）

```json
{
  "date": "2026-01-05",
  "venue": "Sse",
  "calendar_revision": "sse-trading-days-2026-v1",
  "announcement_url": "https://www.sse.com.cn/disclosure/announcement/general/c/c_20251222_10802507.shtml",
  "holiday_calendar_url": "https://www.gov.cn/gongbao/2025/issue_12406/material/gwygb202532.pdf",
  "date_basis": "SseAnnouncedClosuresAndWeekdays",
  "evidence": {
    "provider": "Sse",
    "source_at": "2025-12-22",
    "observed_at": "1759... .123456789",
    "batch_id": "sse-trading-days-2026-v1"
  }
}
```

- 每次请求返回该年**全部**交易日（2026 = 242 条），严格递增、无重复、全部为周一至周五；
- `announcement_url` 为上交所年度休市安排公告（上证公告〔2025〕45号）；
  `holiday_calendar_url` 为国务院办公厅 2026 年节假日安排（公报 PDF）；
- `date_basis` 说明日期来自"已公告休市段 + 工作日"的评审期推导。

## 3. 证据语义（本族特有，必须按原义消费）

本族是仓库第一个**前瞻性已确定**的数据族：修订在生成时刻即对全部未来日期成立。
因此：

- `evidence.source_at` = **公告自身的发布时刻**（2025-12-22，SSE 公告发布日期），
  **不是**"某条记录被观测到的时间"；批次（`QueryResponse.source_at`）与逐条 evidence
  均携带同一值；
- `observed_at` = 修订服务的本地生成/读取时刻（epoch 秒.纳秒文本）；
- `batch_id` = 修订号 `sse-trading-days-2026-v1`；
- 其他族的 source_at 语义（该值的观测时刻）**不适用于本族**，跨族 join 时不得混用。

## 4. 失败语义

| 情形 | 结果 |
| --- | --- |
| 非 2026 年份 | `Unsupported`（gRPC `UNIMPLEMENTED`），无 I/O |
| `Szse` 场所 | `Unsupported`，无 I/O |
| schema/version 不匹配 | `INVALID_ARGUMENT` |
| 合法请求 | `ADMITTED`、`complete=true`、242 条记录 |

## 5. 边界与残留限制

- 本日历判定"该日交易所是否开市"，**不能**单独判定"某证券某日应否有行情"；停牌是
  证券级事实，需另有来源（设计动机即"停牌 vs 源缺陷"的集合减法）；
- 不含盘中临时休市/延迟开闭市；此类调整出现时必须新增修订；
- 不含港股通/北交所差异日；
- 2025 及后续年份未准入；运行时永不按日历公式外推（BR-051 先例的禁令适用于本族）；
- 构造、复核与准入证据见仓库内
  `docs/evidence/2026-10-15-sse-trading-calendar-revision.md`；
  复核工具 `tools/probes/trading-calendar-crosscheck.py`。

## 6. 兼容性

- v1 为初始版本；新增字段按仓库 per-family 版本先例升级版本号（参考
  `FUTURES_DELIVERY_SCHEMA_VERSION`、`SECURITY_PROFILES_SCHEMA_VERSION`）；
- 本 op 不影响任何既有 operation 的合同；新增 op 更改 `contract_sha256`，
  对端需在重新生成的 client bundle 上协同冻结。
