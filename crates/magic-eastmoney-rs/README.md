# magic-eastmoney-rs

`magic-eastmoney-rs` is a read-only adapter for a bounded set of Eastmoney
public-web market-data endpoints. Its provider identity is
`ProviderId::Eastmoney` and its provenance source is `eastmoney-web`.
It is deliberately separate from the authenticated Choice/EMQuant adapter.

The public endpoints used here do not publish a project-visible stability SLA.
Deploy only where public-web automation is authorized, keep a fallback source,
and monitor parser failures. The crate never reads browser cookies, account
sessions, credentials, portfolios, or order data.

## Implemented contracts

| Family | Core contract | Public host | Verified mapping |
| --- | --- | --- | --- |
| Instrument and industry reports | `ResearchReports` | `reportapi.eastmoney.com` | title, institution, author, rating, industry, publication time, EPS forecasts, PDF URL |
| Instrument fund flow | `FundFlowSeries` | `push2.eastmoney.com`, `push2his.eastmoney.com` | minute and daily parsers/mapping implemented, but `fund_flow_series=false` until a successful live admission probe |
| Board fund flow | `BoardFlows` | `push2.eastmoney.com` | industry/concept/region; 1/5/10-day ranking, return, main flow, daily tiers, leader when supplied |
| Dragon-tiger list | `DragonTigerData` | `datacenter-web.eastmoney.com` | entries plus buy/sell seats, amounts, reason, turnover and independent side ranks |
| Margin | `MarginData` | `datacenter-web.eastmoney.com` | financing and securities-lending balances, buys, repayments and quantities |
| Block trades | `BlockTrades` | `datacenter-web.eastmoney.com` | price, close, premium, volume, amount, buyer and seller |
| Holder counts | `HolderCounts` | `datacenter-web.eastmoney.com` | holders, change, ratio and average free shares |
| Lockups | `LockupEvents` | `datacenter-web.eastmoney.com` | type, listing date, shares, able shares, free-float ratio and value |
| Dividends | `DividendPlans` | `datacenter-web.eastmoney.com` | report/ex-date, state, cash/bonus/transfer/allotment per ten shares |
| Limit pools | `LimitPools` | `push2ex.eastmoney.com` | upper, broken, lower and previous-upper pools; source `qdate` is mandatory and must match the requested date |
| Popularity | `PopularityData` | `emappdata.eastmoney.com`, `push2.eastmoney.com` | rank and rank change, with separately evidenced quote join |

Unsupported operations fail explicitly. In particular, generic board flow is
not advertised as the exact 15:35 post-close Top-10 contract. Consensus,
semantic search, PDF downloading, global news, announcements, investor
questions, instrument news, inferred reasons, and account-backed data remain
disabled.
The callable fund-flow method is retained for deterministic fixtures and
diagnostics, but it is not an admitted capability because neither public host
has completed a successful live probe on this environment.
Eastmoney's public keyword-news search is also unadmitted: its real result rows
do not contain a structured source instrument identity. A query keyword is not
promoted into `NewsItem::instruments`; `instrument_news` returns a typed
`Unsupported` error and `ContentCapabilities::instrument_news` is false.

## Safety and transport bounds

- HTTPS is mandatory.
- Only the six exact Eastmoney public API hosts in the transport allowlist are
  reachable.
- Redirect following is disabled.
- Successful responses must carry one of the documented JSON/JSONP media
  types; missing content type, HTML, and arbitrary binary payloads fail before
  parsing.
- Connect, read, and write timeouts default to 12 seconds.
- A response is capped at 4 MiB and a POST body at 64 KiB.
- Production clones share one limiter and issue at most one request per second.
- Datacenter pagination is sequential, capped at 10,000 records, always uses a
  remote page size of 500, and truncates only after complete pages are merged.
  This preserves server offsets for requests such as `limit=700`.
- Documented empty statuses are recognized, but the provider facade still
  rejects every empty batch as an explicit protocol failure; malformed shapes
  and other error codes also fail.

An injected `EastmoneyTransport` supports deterministic fixtures without
network, cookies, or hidden global state.

## Units and provenance

- Fund-flow and most capital amount fields remain yuan.
- Eastmoney limit-pool price integers are divided by 1,000.
- Lockup share counts supplied in ten-thousand shares are multiplied by 10,000.
- Lockup `LIFT_MARKET_CAP` supplied in ten-thousand yuan is multiplied by
  10,000 into CNY.
- Percentage-valued fields remain `RatioUnit::Percent`; block-trade
  `PREMIUM_RATIO` and lockup `FREE_RATIO` preserve the source decimal as
  `RatioUnit::Decimal`. Absence is `None`, never synthetic zero.
- Dividend cash/bonus/transfer/allotment fields are retained per ten shares.
- Limit-pool records and evidence use the response's calendar-valid `qdate`,
  never the request date by assumption. Seal timestamps must be valid
  `HH:MM:SS` clocks.
- Every record carries `SourceEvidence` with `ProviderId::Eastmoney`, observed
  time, batch ID, and source time when the response proves one.
- Board-flow batches require the source-provided positive Unix update timestamp
  `f124`; every row in one atomic batch must carry the same value.
- Popularity ranking evidence and the optional quote-join evidence have
  different batch IDs.
- Caller instruments must have a verified A-share code prefix matching their
  declared exchange. Source code/market identities are independently parsed
  and cross-checked; duplicate popularity ranks, instruments, or quote codes
  are protocol failures rather than last-write-wins joins.

## Usage

```rust
use magic_eastmoney_rs::EastmoneyClient;
use magic_market_core::{
    AssetClass, Exchange, FlowInterval, FlowScope, FundFlowRequest,
    FundFlowSeries, InstrumentId, PositiveU32,
};

let client = EastmoneyClient::new()?;
let instrument =
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity)?;
let request = FundFlowRequest::new(
    FlowScope::Instrument(instrument),
    FlowInterval::Minute1,
    PositiveU32::new(20)?,
)?;
let batch = client.fund_flow_series(&request)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Verification

Deterministic gates:

```text
cargo fmt -p magic-eastmoney-rs -- --check
cargo test -p magic-eastmoney-rs --all-targets --locked --offline
cargo clippy -p magic-eastmoney-rs --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc -p magic-eastmoney-rs --no-deps --locked --offline
```

The bounded live probe prints capabilities, every normalized field, provenance,
quality, and every record for every admitted family:

```text
cargo run -p magic-eastmoney-rs --example live_probe --release --locked
```

Fund-flow and keyword-only instrument news are still called as unadmitted
diagnostics. Their failures are printed as
`diagnostic_status=expected_failure` and do not turn an otherwise successful
admitted-family probe into a false failure.

Optional environment:

```text
MAGIC_EASTMONEY_CODE=600396
MAGIC_EASTMONEY_REFERENCE=600519
MAGIC_EASTMONEY_EVENT_CODE=002475
MAGIC_EASTMONEY_REPORT_CODE=688017
MAGIC_EASTMONEY_INDUSTRY=*
MAGIC_EASTMONEY_POOL_DATE=2026-07-23
```

The load probe is deliberately serial. It rejects concurrency other than one
and pacing below one second, prints every returned record, and reports success
high-level attempt count and throughput, min/p50/p95/max attempt latency, and
the minimum high-level attempt start gap:

```text
MAGIC_EASTMONEY_LOAD_REQUESTS=6 \
MAGIC_EASTMONEY_LOAD_CONCURRENCY=1 \
MAGIC_EASTMONEY_LOAD_PACING_MS=1000 \
MAGIC_EASTMONEY_LOAD_OPERATION=mixed \
cargo run -p magic-eastmoney-rs --example load_probe --release --locked
```

`MAGIC_EASTMONEY_LOAD_OPERATION` accepts `mixed`, `research`, `fund-flow`,
`board-flow`, `limit-pool`, `popularity`, or `news`. `mixed` rotates only
admitted families; explicit `fund-flow` and `news` remain available as
diagnostics.
High-level attempts are capped at 20. The summary includes attempts/second,
min/P50/P95/P99/max attempt latency, explicit pacing-wait total/P95, minimum
attempt start gap, status counts, and typed error categories. These are not
transport HTTP-request metrics: some families issue more than one HTTP request
per high-level attempt. Machine-readable fields therefore use
`high_level_attempts`, `attempts_per_second`, and `attempt_latency_*`; they do
not claim HTTP request counts or HTTP RPS. Explicit `fund-flow` and `news` runs
print `admitted=false` for every attempt. Any failed diagnostic exits nonzero
with `diagnostic_status=diagnostic_failed`; a successful transport/parser check
is still unadmitted and is reported only as
`diagnostic_status=diagnostic_complete_unadmitted`.

## Live verification on 2026-07-23

The full release probe returned real records for instrument and industry
reports, industry/concept/region board flows, dragon-tiger entries and seats,
margin, block trades, holder counts, lockups, dividends, all four limit-pool
kinds, and popularity plus quote join. Example samples included `688017`
reports, `002475` dragon-tiger records, and `600396` popularity. All four
limit-pool responses supplied a matching source `qdate`.

The public news endpoint returned real keyword-search hits, but the rows did
not contain structured instrument identity. It is therefore reported only as
an unadmitted `Unsupported` diagnostic, not as an instrument-news live pass.

The current machine/IP could not complete either fund-flow transport:

- `push2.eastmoney.com` minute flow closed the connection before an HTTP status;
- `push2his.eastmoney.com` daily flow did the same;
- exact reference-shape `curl` checks returned `Empty reply from server`;
- deterministic minute/daily field and unit fixtures still pass.

This is reported as a typed transport failure. `fund_flow_series` therefore
remains false. It is not counted as a live pass, converted into an empty batch,
or replaced with another data family.

The historical six-operation run included fund flow and therefore reported
5 successes / 1 explicit failure. Current `mixed` load runs intentionally
exclude fund flow and keyword-only news; select either explicitly to exercise
its unadmitted diagnostic boundary.

## Deployment

The crate has no SDK, dynamic library, browser, account, or sidecar dependency.
Package the Rust binary normally and provide:

1. a system CA bundle usable by the TLS backend;
2. outbound TCP/443 access only to the documented host allowlist;
3. logs/metrics for typed transport, decode, protocol, unsupported, and core
   errors;
4. a caller-side cache and provider fallback suitable for an endpoint with no
   public SLA;
5. process-level rate coordination if multiple independent processes are
   deployed, because the built-in limiter is shared only by clones in one
   process.

Do not copy Choice/EMQuant activation files into this crate and do not route
authenticated client traffic through it.
