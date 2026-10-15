# SSE trading-calendar revision `sse-trading-days-2026-v1`：构造、复核与准入证据（2026-10-15）

本记录对应 Gate B 实施的签入修订 `sse-trading-days-2026-v1`（`TradingCalendar` op，
SSE 场所，2026 年 242 个交易日）。修订是仓库内第一个"前瞻性已确定"的数据族：发布时即
对全部未来日期成立。本记录保存构造口径、逐段核对结果、机械交叉验证和准入探针结果；
原始探针日志与交叉验证 JSON 见会话 scratch 目录（`%LOCALAPPDATA%\hermes\cache\scratch\trading-calendar\`），
命令在下文可复现。

## 1. 权威来源

| 来源 | 标识 | 发布 | URL |
| --- | --- | --- | --- |
| 上交所年度休市安排（主口径） | 上证公告〔2025〕45号 | 2025-12-22 | https://www.sse.com.cn/disclosure/announcement/general/c/c_20251222_10802507.shtml |
| 上交所"休市安排"汇总页 | 年度休市安排 2026 | — | http://www.sse.com.cn/disclosure/dealinstruc/closed/ |
| 国务院办公厅 2026 年部分节假日安排 | 国办发明电〔2025〕7号 | 2025-11-04 | https://www.gov.cn/gongbao/2025/issue_12406/material/gwygb202532.pdf |

边界（设计 §5）：交易所不发布机器可读的"交易日清单"；交易日 = 工作日 − 已公告休市日，
是**派生结果**。本修订的权威级别是"依官方公告、经复核的修订"，不是源端发布的向量，
op 描述、合同文档与 `admissions.tsv` 备注三处均已写明。

## 2. 构造口径（仅发生在评审环节，运行时零公式）

以 SSE 公告的休市段为闭集，从 2026 年全部周一至周五中剔除（公告含调休上班的周六：
2/14、2/28 仍为周末休市，不构成交易日）：

| 节假日 | 休市段（闭区间） | 复市日 |
| --- | --- | --- |
| 元旦 | 2026-01-01 .. 2026-01-04 | 2026-01-05 |
| 春节 | 2026-02-14 .. 2026-02-23 | 2026-02-24 |
| 清明节 | 2026-04-04 .. 2026-04-06 | 2026-04-07 |
| 劳动节 | 2026-05-01 .. 2026-05-05 | 2026-05-06 |
| 端午节 | 2026-06-19 .. 2026-06-21 | 2026-06-22 |
| 中秋节 | 2026-09-25 .. 2026-09-27 | 2026-09-28 |
| 国庆节 | 2026-10-01 .. 2026-10-07 | 2026-10-08 |

结果：242 个交易日，2026-01-05 为首日，2026-12-31 为末日。
日期向量摘要（逐日 `YYYY-MM-DD` 以 `\n` 连接后 SHA-256）：
`d45fea805c6312bc11943d70336929117f5faf946efd1dca9d0e6f5499f7113d`。

确定性测试（`crates/magic-exchange-rs/src/trading_calendar.rs`）从同一公告休市段
独立复核向量：数量、首末日、严格递增、全为工作日、各休市段内无成员、13 个边界日
在场；反向测试证明 2025/2027/2030 与 Szse 场所在任何工作之前返回 typed `Unsupported`。

## 3. 实盘机械交叉验证（本地终端）

本机 TDX 终端（TdxW.exe，TQ-Local `http://127.0.0.1:17709/`）以
`get_trading_dates`（market=SH）返回其自身的上交所交易日向量。2026-10-15 实测：

```text
OK: terminal list is an exact prefix of the sealed 2026 vector (186/242 days, terminal through 2026-10-14)
```

即：终端已确认的 186 个日期与封存向量逐位相同，边界连续（2026-10-14 之后的下一位
正是 2026-10-15；当日日期要等终端完成盘后数据下载后才入列）。复核工具为
`tools/probes/trading-calendar-crosscheck.py`（只读 loopback、零凭据、失败退出码 1），
后续每次修订都应先跑该工具。

## 4. 准入探针（正式 trait，零网络）

`magic-exchange-rs` 的 `live_probe` 新增 `sse-trading-calendar` 模式，直接调用正式
`TradingCalendar` trait（无任何 transport 参与）。2026-10-15 完成 **2 次 live + 3 次
串行**：

```bash
MAGIC_EXCHANGE_LIVE_OPERATION=sse-trading-calendar \
cargo run -p magic-exchange-rs --example live_probe --release --locked --offline
```

五次运行全部 `exit=0`、`live_probe_status=admitted`、`records=242`、
`first_date=2026-01-05 last_date=2026-12-31`、`quality.complete=true`、
`provenance.source_at=Some("2025-12-22")`、`batch_id=sse-trading-days-2026-v1`。

反向探针（同样 5 次运行之外的显式负例）：

```text
MAGIC_SSE_CALENDAR_YEAR=2025  -> Error: Unsupported("formal SSE trading calendar is admitted only for 2026; requested 2025")
MAGIC_SSE_CALENDAR_VENUE=Szse -> Error: Unsupported("formal trading calendar is admitted only for venue Sse; Shenzhen applicability is not established")
```

composition 层另有两个公开回归（`grpc_production_coverage_tests.rs`）：
`public_2026_query_returns_the_reviewed_vector_with_notice_evidence`（调用方模型严格解析
全部 242 条记录、逐条 evidence、首末日与公告 URL）与
`other_years_and_venues_fail_closed_at_the_provider_boundary`（typed 拒绝）。

## 5. 残留限制（不写入合同）

- 不含盘中临时休市/延迟开闭市等临时公告调整；出现时必须新增修订、复核、测试并发布；
- 不含个股停牌——本日历只能判定"该日交易所是否开市"，不能单独判定"某证券某日应否有行情"；
- 不含港股通/北交所差异日；SSE 与 SZSE 实务同历属于假设，SZSE 适用性未在此建立；
- 不含 2025 及后续年份：运行时永不按公式外推，非 2026 年份一律 typed 拒绝。
