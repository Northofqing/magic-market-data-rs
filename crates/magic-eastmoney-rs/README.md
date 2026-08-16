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
| Instrument and industry reports | `ResearchReports`, `ResearchDocuments` | `reportapi.eastmoney.com`, `pdf.dfcfw.com` | metadata plus the exact original bounded PDF body |
| Report target-price aggregation | `TargetPriceData` | `reportapi.eastmoney.com` | complete pagination; source code and `stockName`; exact `indvAimPriceL/T`; arithmetic mean of report range midpoints; typed verified-empty result for the exact all-zero shape |
| Instrument fund flow | `FundFlowSeries` | `push2.eastmoney.com`, `push2his.eastmoney.com` | minute and daily parsers/mapping implemented, but `fund_flow_series=false` until a successful live admission probe |
| Board fund flow | `BoardFlows` | `push2.eastmoney.com` | industry/concept/region; 1/5/10-day ranking, return, main flow, daily tiers, leader when supplied |
| Dragon-tiger list | `DragonTigerData` | `datacenter-web.eastmoney.com` | entries plus one atomic buy-five/sell-five seat group, amounts, reason, turnover and independent side ranks; seat limit must be at least 10 |
| Full-market dragon-tiger discovery | `DragonTigerDiscovery` | `datacenter-web.eastmoney.com` | complete dated pagination across Shanghai/Shenzhen/Beijing, stable source ID, stock code and name |
| Full-market dragon-tiger seats | `MarketDragonTigerData` | `datacenter-web.eastmoney.com` | explicit-date whole-A-share discovery; source `TRADE_ID` keeps same-stock reasons distinct and binds each entry to one atomic buy-five/sell-five group |
| Margin | `MarginData` | `datacenter-web.eastmoney.com` | financing and securities-lending balances, buys, repayments and quantities |
| Block trades | `BlockTrades` | `datacenter-web.eastmoney.com` | price, close, premium, volume, amount, buyer and seller |
| Holder counts | `HolderCounts` | `datacenter-web.eastmoney.com` | holders, change, ratio and average free shares |
| Lockups | `LockupEvents` | `datacenter-web.eastmoney.com` | type, listing date, shares, able shares, free-float ratio and value |
| Dividends | `DividendPlans` | `datacenter-web.eastmoney.com` | report/ex-date, state, cash/bonus/transfer/allotment per ten shares |
| Limit pools | `LimitPools` | `push2ex.eastmoney.com` | upper, broken, lower and previous-upper pools; source `qdate` is mandatory and must match the requested date |
| Popularity | `PopularityData` | `emappdata.eastmoney.com`, `push2.eastmoney.com` | rank and rank change, with separately evidenced quote join |
| Strict post-close ranking diagnostic | `EastmoneyClient::diagnose_post_close_flows` | `push2.eastmoney.com`, `push2delay.eastmoney.com` | current China date after 15:35, exact limit, one source timestamp, contiguous rank, code and name; production capability is false and formal `PostCloseFlows` returns `Unsupported` |
| Full-market rankings | `MarketRankings` | `push2.eastmoney.com`, `push2delay.eastmoney.com` | complete A-share pagination for volume ratio and main-net inflow, including code, name, source session, three-market coverage and one common source time (zero skew); a transport failure discards all pages and restarts at page one on the alternate HTTPS host; capability stays false until a stable-session live probe satisfies every gate |
| Provider Top-N rankings | `ProviderTopNRankings` | `push2.eastmoney.com`, `push2delay.eastmoney.com` | one provider-ordered page after 15:35 on the requested date or on a later closed-market capture date; exact selected metric/code/name, local source-response ordinal, per-security `f297` equal to the requested settled date, provider-declared total, inspected row count and no fabricated batch `source_at`; this is not arbitrary history, full-market coverage or breadth |
| Global latest finance news | `NewsProvider::global_news` | `roll.eastmoney.com`; metadata links on exact `finance`, `global`, or `biz.eastmoney.com` hosts | complete first-page validation, newest-first minute time, numeric article identity and canonical URL; article pages are not fetched |

Unsupported operations fail explicitly. Provider-published earnings consensus, semantic search,
announcements, investor questions, instrument news, inferred reasons, and
account-backed data remain disabled.

Ranking admission is independent by metric:

| Metric | `MarketRankingCapabilities` | Current production admission |
| --- | --- | --- |
| Volume ratio | `volume_ratio` | `false` |
| Main-net inflow | `main_net_inflow` | `false` |

The legacy aggregate `SignalCapabilities.market_rankings` becomes true only
when both metrics are admitted. Each ranking page retries at most three times
and only for typed transport failures; every retry still passes through the
shared one-request/second gate. Endpoint failover never joins pages from two
hosts: a failed partial operation is discarded and the alternate host starts
again at page one. Intraday page drift remains an explicit atomic failure.
`CapitalCapabilities.post_close_flow` is also false. The formal
`PostCloseFlows` implementation returns typed `Unsupported`; only the explicitly
named diagnostic method performs network I/O. A diagnostic success prints
`admitted=false` and still requires human review before the capability can be
enabled.

Provider Top-N admission is a separate capability family:

| Metric | `ProviderTopNRankingCapabilities` | Current production admission |
| --- | --- | --- |
| Volume ratio | `volume_ratio` | `true` |
| Main-net inflow | `main_net_inflow` | `true` |

These values do not change `MarketRankingCapabilities` or
`SignalCapabilities.market_rankings`; both complete-market metrics remain
false. Provider Top-N accepts at most 100 records from exactly one response and
must be described as “Eastmoney single-response provider-ordered Top-N with
the requested metric present in every returned row.” It must not feed market
breadth or claim complete-universe coverage.
Consumers build requests with
`EastmoneyClient::provider_top_n_a_share_request`; the provider crate owns the
canonical A-share filter wire grammar. The concrete composition constructor
uses `EastmoneyClient::provider_top_n_source_identity` and
`EastmoneyClient::provider_top_n_ranking_capabilities`; it creates the
production `EastmoneyClient` internally and exposes neither client injection
nor generic registration, so downstream code cannot substitute an injected
transport or impersonate source/capability ownership.
The callable fund-flow method is retained for deterministic fixtures and
diagnostics, but it is not an admitted capability because neither public host
has completed a successful live probe on this environment.
Eastmoney's public keyword-news search is also unadmitted: its real result rows
do not contain a structured source instrument identity. A query keyword is not
promoted into `NewsItem::instruments`; `instrument_news` returns a typed
`Unsupported` error and `ContentCapabilities::instrument_news` is false.

## Safety and transport bounds

- HTTPS is mandatory.
- Only the exact Eastmoney public API/PDF hosts in the transport allowlist are
  reachable.
- Redirect following is disabled.
- API responses must carry a documented JSON/JSONP media type. Research
  documents must be `application/pdf`, start with `%PDF-`, match the exact
  report identity and stay within 32 MiB; HTML and arbitrary binary payloads
  fail before admission.
- Global news uses only the exact
  `https://roll.eastmoney.com/finance.html` page. It requires UTF-8 HTML,
  stays within 2 MiB, validates every first-page row before truncation, and
  accepts only canonical numeric finance article identities.
- Connect, read, and write timeouts default to 12 seconds.
- A response is capped at 4 MiB and a POST body at 64 KiB.
- Production clones share one limiter and issue at most one request per second.
- Datacenter pagination is sequential, capped at 10,000 records, always uses a
  remote page size of 500, and truncates only after complete pages are merged.
  This preserves server offsets for requests such as `limit=700`.
- Target-price `hits=0,size=0,TotalPage=0,data=[]` is returned as typed
  `VerifiedEmpty` carrying exact request identity and batch evidence.
  Partial-zero contradictions, malformed shapes, and other error codes fail.

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
- Every target-price observation requires the source code and non-empty
  `stockName`; all observations in one aggregate must agree. `indvAimPriceL`
  is retained as the lower bound and `indvAimPriceT` as the upper bound. The
  exposed `mean` is the arithmetic mean of report range midpoints
  `(L + T) / 2`, not a provider-published or contributor-weighted consensus.

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

Isolated target-price, full-market ranking, and unadmitted post-close
diagnostics:

```text
MAGIC_EASTMONEY_LIVE_OPERATION=target-price \
MAGIC_EASTMONEY_TARGET_CODE=600519 \
MAGIC_EASTMONEY_TARGET_FROM=2026-01-01 \
MAGIC_EASTMONEY_TARGET_THROUGH=2026-07-27 \
cargo run -p magic-eastmoney-rs --example live_probe --release --locked

MAGIC_EASTMONEY_LIVE_OPERATION=market-rankings \
MAGIC_EASTMONEY_RANKING_KIND=all \
cargo run -p magic-eastmoney-rs --example live_probe --release --locked

MAGIC_EASTMONEY_LIVE_OPERATION=provider-topn-rankings \
MAGIC_EASTMONEY_TOPN_DATE=<current-Asia/Shanghai-date> \
MAGIC_EASTMONEY_RANKING_KIND=all \
cargo run -p magic-eastmoney-rs --example live_probe --release --locked

MAGIC_EASTMONEY_LIVE_OPERATION=post-close-ranking \
MAGIC_EASTMONEY_POST_CLOSE_DATE=2026-07-27 \
MAGIC_EASTMONEY_POST_CLOSE_LIMIT=20 \
cargo run -p magic-eastmoney-rs --example live_probe --release --locked
```

The post-close operation calls only the diagnostic method and always prints
`admitted=false`. It rejects calls before `15:35:00 Asia/Shanghai`, a date other
than the current China date, missing names, incomplete ranks, or mixed source
timestamps. The formal trait never performs this fetch while capability is
false.

Fund-flow and keyword-only instrument news are still called as unadmitted
diagnostics. Global latest finance news is an admitted family. Diagnostic
failures are printed as
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
MAGIC_EASTMONEY_DRAGON_TIGER_DATE=2026-07-23
```

The two source-session dates are required by the full live probe. For an
isolated whole-market dragon-tiger check, set
`MAGIC_EASTMONEY_DRAGON_TIGER_LIMIT` (default `5`, maximum `100`) and run the
`market_dragon_tiger_probe` example.

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
admitted families and includes `news`; only explicit `fund-flow` remains a
diagnostic.
High-level attempts are capped at 20. The summary includes attempts/second,
min/P50/P95/P99/max attempt latency, explicit pacing-wait total/P95, minimum
attempt start gap, status counts, and typed error categories. These are not
transport HTTP-request metrics: some families issue more than one HTTP request
per high-level attempt. Machine-readable fields therefore use
`high_level_attempts`, `attempts_per_second`, and `attempt_latency_*`; they do
not claim HTTP request counts or HTTP RPS. Explicit `fund-flow` runs print
`admitted=false` for every attempt. Any failed diagnostic exits nonzero
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

The keyword-search endpoint returned real hits but no structured instrument
identity, so it remains an unadmitted `instrument_news` diagnostic. Separately,
the official finance rolling page backs admitted `global_news`; these records
carry category and canonical article identity but intentionally have no
instrument identity.

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
exclude fund flow but include global latest news. Select `fund-flow` explicitly
to exercise its unadmitted diagnostic boundary.

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
