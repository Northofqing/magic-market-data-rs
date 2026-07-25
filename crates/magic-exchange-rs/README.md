# magic-exchange-rs

Read-only adapters for admitted SSE, SZSE, HKEX and CFFEX official public data.
The crate keeps the four venues as separate Provider identities and exposes only
families that have deterministic fixtures plus a successful production-trait
probe.

## Admitted capabilities

| Client | Core trait | Exact HTTPS endpoint |
| --- | --- | --- |
| `SseClient` | `Announcements` | `query.sse.com.cn/security/stock/queryCompanyBulletin.do` |
| `SseClient` | `DragonTigerData` | `query.sse.com.cn/infodisplay/showTradePublicFile.do` |
| `SzseClient` | `Announcements` | `www.szse.cn/api/disc/announcement/annList` |
| `SzseClient` | `RealtimeQuotes`, `OrderBooks` | `www.szse.cn/api/market/ssjjhq/getTimeData` |
| `SzseClient` | `DragonTigerData` | `www.szse.cn/api/report/ShowReport/data` |
| `HkexClient` | `NorthboundDailyStatistics` | `www.hkex.com.hk/eng/csm/DailyStat/data_tab_daily_<YYYYMMDD>e.js` |
| `CffexClient` | `FuturesDeliveryCalendar` | `www.cffex.com.cn/jystz/` and dated same-host notice details |

SSE/SZSE announcements validate complete remote pages before local
truncation. Dragon-tiger requests require an explicit trading date. SZSE
fetches every declared list page, verifies stable totals and global entry-ID
uniqueness, then fetches selected detail pages. Seat results are atomic
buy-five/sell-five groups; a caller limit below ten is rejected and a larger
non-multiple is rounded down to complete groups.

SZSE Quote/order-book responses preserve the source quantity values in `手`;
Core `Quantity` currently has no unit tag, so the Provider does not multiply
them by 100. Missing tail levels produce `DataStatus::Unavailable` and an
incomplete batch quality report. HKEX DailyStat preserves CNY totals, trade
counts, ETF turnover, exact Top10 ranks and the quota `999,999,999` sentinel as
`NorthboundQuotaBalance::Unavailable`.

CFFEX scans at most 120 official notice-list pages for the requested contract
month. A detail is admitted only when its title, delivery-settlement wording,
IF/IH/IC/IM contract identities, actual delivery date, requested month, and
cash-settlement semantics agree. Holiday shifts come from the notice text;
the Provider never substitutes a “third Friday” formula.

All transports enforce credential-free HTTPS host/path allowlists, port 443,
zero redirects, exact final URLs, bounded content types, an 8 MiB response
ceiling and 1–60 second timeouts. Client clones share a serial request gate and
hold it through the complete response read; request starts are at least one
second apart.

## Probes

```bash
cargo run -p magic-exchange-rs --example live_probe --release --locked --offline

MAGIC_EXCHANGE_LIVE_OPERATION=cffex-delivery \
MAGIC_CFFEX_DELIVERY_YEAR=2026 \
MAGIC_CFFEX_DELIVERY_MONTH=2 \
cargo run -p magic-exchange-rs --example live_probe --release --locked --offline

MAGIC_EXCHANGE_LOAD_REQUESTS=8 \
MAGIC_EXCHANGE_LOAD_CONCURRENCY=1 \
MAGIC_EXCHANGE_LOAD_PACING_MS=1000 \
cargo run -p magic-exchange-rs --example load_probe --release --locked --offline
```

Defaults:

- announcements: `600396.SH` and `000858.SZ`;
- SSE dragon-tiger: `600396`, `2026-07-22`;
- SZSE dragon-tiger: `000603`, `2026-07-23`;
- HKEX DailyStat: `2026-07-22`, both northbound channels.
- CFFEX delivery notice: `2026-02`, exactly IF2602/IH2602/IC2602/IM2602.

Override with `MAGIC_EXCHANGE_SSE_CODE`, `MAGIC_EXCHANGE_SZSE_CODE`,
`MAGIC_EXCHANGE_SSE_DRAGON_DATE`, `MAGIC_EXCHANGE_SZSE_DRAGON_CODE`,
`MAGIC_EXCHANGE_SZSE_DRAGON_DATE`, `MAGIC_EXCHANGE_HKEX_DATE` and
`MAGIC_EXCHANGE_LIVE_LIMIT`. Set
`MAGIC_EXCHANGE_LIVE_OPERATION=cffex-delivery` for the isolated CFFEX probe and
override its month with `MAGIC_CFFEX_DELIVERY_YEAR` and
`MAGIC_CFFEX_DELIVERY_MONTH`.

The final 2026-07-23 production-trait live probe passed announcements,
SSE/SZSE dragon-tiger entries and complete seats, SZSE Quote/five-level book,
and both HKEX northbound channels. The final serial mixed load run passed 8/8
high-level attempts:

```text
attempts=8 successes=8 failures=0
measurement_elapsed_ms_excluding_output=7510
operation_elapsed_total_ms=2771 pacing_wait_total_ms=4738
attempt_throughput_per_second=1.0652
attempt_latency_min_ms=36 attempt_latency_p50_ms=120
attempt_latency_p95_ms=1201 attempt_latency_p99_ms=1201
attempt_latency_max_ms=1201 minimum_attempt_start_gap_ms=1000
load_probe_status=passed
```

The isolated CFFEX production-trait probe also returned exactly four
cash-delivery events for IF2602, IH2602, IC2602, and IM2602 from the official
notice; the actual date was parsed from the notice rather than derived from a
calendar formula.

These are bounded connectivity/validation measurements, not exchange SLA or
permission for sustained collection. One high-level attempt can issue
multiple internally paced HTTP requests.

## Deterministic gates

The repository follows the current stable toolchain and declares no fixed
MSRV.

```bash
cargo test -p magic-exchange-rs --all-targets --locked --offline
cargo clippy -p magic-exchange-rs --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc -p magic-exchange-rs --no-deps --locked --offline
```

No client reads cookies, account state or desktop-terminal traffic. There is
no plaintext, obsolete-TLS or fixture fallback in production.
