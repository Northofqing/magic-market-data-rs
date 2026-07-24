# Sina Public Market Data Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a strictly validated Sina public-web provider for沪深京 A-share quotes, five-level books, selected K lines, current minute data and partial security metadata, with live/load proof and release packaging.

**Architecture:** A cloneable `SinaClient` owns one bounded `ureq` transport. `lib.rs` handles symbols and GB18030 snapshots, `bars.rs` handles Sina JSON K lines, and `minute.rs` derives cumulative latest-session minute points from verified one-minute rows. Every record keeps `ProviderId::Sina`, source time, observation time and batch identity; unsupported families remain disabled.

**Tech Stack:** Rust stable, `magic-market-core`, `ureq 2.12.1`, `encoding_rs`, `serde_json`, deterministic fixtures, official Sina HTTPS endpoints.

---

## File map

- Create `crates/magic-sina-rs/Cargo.toml`: crate metadata and bounded HTTP/parser dependencies.
- Create `crates/magic-sina-rs/src/lib.rs`: client, transport, symbol validation, Quote/OrderBook/metadata parsing.
- Create `crates/magic-sina-rs/src/bars.rs`: K-line interval mapping, JSON validation and `HistoricalBars`.
- Create `crates/magic-sina-rs/src/minute.rs`: latest-session accumulation and `MinuteData`.
- Create `crates/magic-sina-rs/tests/capabilities.rs`: public trait/capability consistency.
- Create `crates/magic-sina-rs/examples/live_probe.rs`: all supported-family live output.
- Create `crates/magic-sina-rs/examples/load_probe.rs`: bounded concurrent measurement.
- Create `crates/magic-sina-rs/README.md`: crate entry point.
- Create `docs/integrations/sina-web.md`: exact source contract and deployment guide.
- Modify `Cargo.toml`, `Cargo.lock`: workspace integration.
- Modify `README.md`, `docs/DEPLOYMENT.md`, `CHANGELOG.md`: capability/deployment truth.
- Modify `tools/compliance/check.sh`, `tools/release/package.sh`: policy and packaged probes.

### Task 1: Scaffold the crate and lock public capability truth

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/magic-sina-rs/Cargo.toml`
- Create: `crates/magic-sina-rs/src/lib.rs`
- Create: `crates/magic-sina-rs/tests/capabilities.rs`

- [ ] **Step 1: Add the failing public contract test**

```rust
use magic_market_core::{
    HistoricalBars, MinuteData, OrderBooks, RealtimeQuotes, SecurityMetadataProvider,
};
use magic_sina_rs::{SinaClient, SinaError};

#[test]
fn public_client_implements_every_advertised_contract() {
    fn assert_contracts<
        T: RealtimeQuotes<Error = SinaError>
            + HistoricalBars<Error = SinaError>
            + MinuteData<Error = SinaError>
            + OrderBooks<Error = SinaError>
            + SecurityMetadataProvider<Error = SinaError>,
    >() {
    }
    assert_contracts::<SinaClient>();
    let capabilities = SinaClient::capabilities();
    assert!(
        capabilities.quotes
            && capabilities.bars
            && capabilities.minute
            && capabilities.order_book
            && capabilities.security_metadata
    );
    assert!(
        !capabilities.trades
            && !capabilities.fundamentals
            && !capabilities.corporate_actions
            && !capabilities.blocks
            && !capabilities.money_flow
            && !capabilities.auction
    );
}
```

- [ ] **Step 2: Run the test and prove that the crate is absent**

Run:

```bash
cargo test -p magic-sina-rs --test capabilities --offline
```

Expected: failure because package `magic-sina-rs` does not exist.

- [ ] **Step 3: Add the workspace member and crate manifest**

Set the workspace line to include `crates/magic-sina-rs` after Tencent and create:

```toml
[package]
name = "magic-sina-rs"
version = "0.2.0"
edition.workspace = true
license.workspace = true

[dependencies]
encoding_rs = "0.8"
magic-market-core = { path = "../magic-market-core", version = "=0.2.0" }
serde_json = "1"
thiserror = { workspace = true }
ureq = { version = "=2.12.1", default-features = false, features = ["tls"] }

[lints]
workspace = true
```

- [ ] **Step 4: Add the public error, client and capability skeleton**

`lib.rs` must forbid unsafe code and expose:

```rust
#[derive(Debug, thiserror::Error)]
pub enum SinaError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("Sina response decoding failed: {0}")]
    Decode(String),
    #[error("Sina protocol error: {0}")]
    Protocol(String),
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

#[derive(Clone)]
pub struct SinaClient {
    endpoint: String,
    transport: std::sync::Arc<dyn SnapshotTransport>,
}

impl SinaClient {
    pub const fn capabilities() -> magic_market_core::Capabilities {
        magic_market_core::Capabilities {
            quotes: true,
            bars: true,
            minute: true,
            trades: false,
            fundamentals: false,
            corporate_actions: false,
            blocks: false,
            money_flow: false,
            order_book: true,
            auction: false,
            security_metadata: true,
        }
    }
}
```

Keep the temporary `HistoricalBars` and `MinuteData` implementations in
`lib.rs` until Tasks 3 and 4 introduce their modules. Add compile-valid trait
method bodies returning
`SinaError::Unsupported("implementation is not yet connected".into())`; the
next tasks replace each advertised method before any release claim.

- [ ] **Step 5: Run the capability test**

Run:

```bash
cargo test -p magic-sina-rs --test capabilities --offline
```

Expected: one passing test.

- [ ] **Step 6: Commit the scaffold**

```bash
git add Cargo.toml Cargo.lock crates/magic-sina-rs
git commit -m "feat: scaffold Sina market data provider"
```

### Task 2: Implement strict snapshots, quotes, books and metadata

**Files:**
- Modify: `crates/magic-sina-rs/src/lib.rs`

- [ ] **Step 1: Add fixture transport and parser tests**

Use GB18030-encoded fixtures for `sh600396`, `sz000001` and `bj920118`. Assert:

```rust
assert_eq!(snapshot.name.as_deref(), Some("华电辽能"));
assert_eq!(snapshot.source_at.as_deref(), Some("2026-07-23T15:34:59+08:00"));
assert_eq!(snapshot.volume_lots, 3_417_800.59);
assert_eq!(snapshot.amount_yuan, Some(5_352_355_411.0));
assert_eq!(snapshot.bids[0], (Some(16.41), Some(64_092.0)));

let quotes = client.realtime_quotes(&[sh(), sz()]).unwrap();
assert_eq!(quotes.records()[0].provider(), ProviderId::Sina);
assert_eq!(quotes.records()[0].volume().get(), 3_417_800.59);
assert_eq!(quotes.provenance().source(), "sina-web");

let books = client.order_books(&[sz()]).unwrap();
assert_eq!(books.records()[0].bids()[0].price().map(Price::get), Some(11.07));
assert_eq!(
    books.records()[0].bids()[0].quantity().map(Quantity::get),
    Some(2_382.0)
);
```

Add negative cases for invalid GB18030, empty payload, fewer than 33 fields,
duplicate request, duplicate response, missing response, unexpected response,
symbol-key mismatch, malformed timestamp, negative amount, high below low,
quantity without price and response larger than the transport cap.

- [ ] **Step 2: Run the snapshot tests and verify failure**

Run:

```bash
cargo test -p magic-sina-rs --lib --offline
```

Expected: compile/test failures for missing snapshot functions and real trait behavior.

- [ ] **Step 3: Implement bounded transport and symbol validation**

Implement these exact boundaries:

```rust
const DEFAULT_ENDPOINT: &str = "https://hq.sinajs.cn/list=";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_BATCH_SIZE: usize = 50;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const SHARES_PER_LOT: f64 = 100.0;

pub trait SnapshotTransport: Send + Sync {
    fn get(&self, url: &str) -> Result<Vec<u8>, SinaError>;
}
```

`HttpsTransport::get` rejects non-HTTPS, sets
`Referer: https://finance.sina.com.cn/` and
`User-Agent: magic-sina-rs/0.2`, refuses redirects, requires HTTP 200 and reads
at most `MAX_RESPONSE_BYTES + 1`.

`validate_instruments` accepts only unique six-digit `AssetClass::Equity`
instruments and maps Shanghai/Shenzhen/Beijing to `sh`/`sz`/`bj`.

- [ ] **Step 4: Implement strict quote parsing**

Decode with `encoding_rs::GB18030`. Require the exact wrapper key and common
fields 0..32. Parse:

```text
0 name; 1 open; 2 previous close; 3 current; 4 high; 5 low;
8 cumulative shares; 9 amount CNY;
10/11..18/19 bid quantity shares / price;
20/21..28/29 ask quantity shares / price;
30 date; 31 time; 32 status.
```

Compute percent change from current and previous close. Convert every share
quantity using:

```rust
fn shares_to_lots(value: f64) -> Result<f64, SinaError> {
    if !value.is_finite() || value < 0.0 {
        return Err(SinaError::Protocol("share quantity must be finite and non-negative".into()));
    }
    Ok(value / SHARES_PER_LOT)
}
```

Validate calendar/time, price range, finite numbers and exact request
cardinality/order before constructing normalized records.

- [ ] **Step 5: Implement the three Core contracts**

`RealtimeQuotes` creates `Quote::from_parts` with `ProviderId::Sina`.
`OrderBooks` converts `0/0` to `BookLevel::unavailable`, computes exact totals
and uses best-effort quality for missing levels. `SecurityMetadataProvider`
copies name/ST, derives board from code, leaves listing date and source-backed
price-limit rules absent, and records those issues.

Every batch uses:

```rust
let batch_id = format!("sina-web:{observed_at}:{kind}");
let provenance = Provenance::new("sina-web", observed_at)?
    .with_source_at(oldest_source_at)?
    .with_batch_id(batch_id)?;
```

Do not set batch source time unless all snapshot records have one.

- [ ] **Step 6: Run snapshot tests**

Run:

```bash
cargo test -p magic-sina-rs --lib --offline
```

Expected: all snapshot, Quote, OrderBook and metadata tests pass.

- [ ] **Step 7: Commit snapshot support**

```bash
git add crates/magic-sina-rs/src/lib.rs
git commit -m "feat: add strict Sina quotes and order books"
```

### Task 3: Implement selected K-line periods

**Files:**
- Create: `crates/magic-sina-rs/src/bars.rs`
- Modify: `crates/magic-sina-rs/src/lib.rs`

- [ ] **Step 1: Add deterministic K-line tests**

Fixtures must cover an intraday row with amount and a daily row without amount:

```rust
let intraday = br#"[{"day":"2026-07-23 14:55:00","open":"16.410","high":"16.410","low":"16.410","close":"16.410","volume":"1243300","amount":"20402553.0000"}]"#;
let batch = parse_bars_response(
    intraday,
    BarInterval::Minute5,
    1,
    &instrument(),
    "observed",
).unwrap();
assert_eq!(batch.records()[0].volume().get(), 12_433.0);
assert_eq!(batch.records()[0].amount().map(Money::get), Some(20_402_553.0));

let daily = br#"[{"day":"2026-07-23","open":"15.300","high":"16.410","low":"14.850","close":"16.410","volume":"341780059"}]"#;
let batch = parse_bars_response(
    daily,
    BarInterval::Day,
    1,
    &instrument(),
    "observed",
).unwrap();
assert_eq!(batch.records()[0].amount(), None);
```

Add failures for empty arrays, non-array roots, missing keys, non-string
numbers, missing intraday amount, negative volume/amount, bad timestamp,
unordered/duplicate rows, too many rows and inconsistent OHLC.

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test -p magic-sina-rs bars::tests --offline
```

Expected: failure because `bars.rs` is not implemented.

- [ ] **Step 3: Implement interval mapping and request gates**

Add `mod bars;` in `lib.rs` and remove the temporary `HistoricalBars`
implementation there so `bars.rs` is the only implementation.

Map only:

```rust
Minute1 => (1, true),
Minute5 => (5, true),
Minute15 => (15, true),
Minute30 => (30, true),
Hour1 => (60, true),
Day => (240, false),
```

Return `Unsupported` for Week/Month/Year, request date ranges and non-equity
symbols. Reject limits above `MAX_BARS = 800`. Build the percent-encoded-safe
URL only from validated fixed symbol/scale/limit values.

- [ ] **Step 4: Parse and normalize K lines**

Parse each object with required string fields. Use identical `bar_start` and
`bar_end`, `Adjustment::Unadjusted`, `ProviderId::Sina` and a batch ID ending
in `:bars`. Convert volume shares to lots; require intraday amount and allow
daily amount to be absent. Require strict ascending unique source times and set
batch provenance to the latest record source time.

- [ ] **Step 5: Run K-line and crate tests**

Run:

```bash
cargo test -p magic-sina-rs --all-targets --offline
```

Expected: all tests pass.

- [ ] **Step 6: Commit K-line support**

```bash
git add crates/magic-sina-rs/src/bars.rs crates/magic-sina-rs/src/lib.rs
git commit -m "feat: add Sina intraday and daily bars"
```

### Task 4: Derive strictly evidenced current minute data

**Files:**
- Create: `crates/magic-sina-rs/src/minute.rs`

- [ ] **Step 1: Add minute accumulation tests**

Use two current-date rows and one prior-date row:

```rust
let fixture = br#"[
  {"day":"2026-07-22 15:00:00","open":"14.90","high":"14.92","low":"14.90","close":"14.92","volume":"100","amount":"1492"},
  {"day":"2026-07-23 09:30:00","open":"15.30","high":"15.40","low":"15.30","close":"15.40","volume":"1000","amount":"15400"},
  {"day":"2026-07-23 09:31:00","open":"15.40","high":"15.50","low":"15.40","close":"15.50","volume":"2000","amount":"31000"}
]"#;
let batch = parse_current_minutes(fixture, &instrument(), "observed").unwrap();
assert_eq!(batch.records().len(), 2);
assert_eq!(batch.records()[1].cumulative_quantity().get(), 30.0);
assert_eq!(
    batch.records()[1].cumulative_amount().map(Money::get),
    Some(46_400.0)
);
assert_eq!(batch.records()[1].minute_at(), "2026-07-23 09:31");
```

Add failures for empty response, no valid newest date, unordered current rows,
missing amount, numeric overflow/non-finite values and a dated
`MinuteDataRequest`; the dated request must match `SinaError::Unsupported`.

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test -p magic-sina-rs minute::tests --offline
```

Expected: failure because minute accumulation is absent.

- [ ] **Step 3: Implement latest-session accumulation**

Add `mod minute;` in `lib.rs` and remove the temporary `MinuteData`
implementation there so `minute.rs` is the only implementation.

Request `scale=1&ma=no&datalen=300`. Parse through the same checked raw K-line
row representation as bars, find the lexically greatest ISO date, filter only
that date, and accumulate source shares and CNY amount with checked finite
arithmetic.

Create each point as:

```rust
MinutePoint::new(
    instrument.clone(),
    minute_at,
    close,
    Quantity::new(cumulative_shares / 100.0)?,
    Some(Money::new(cumulative_amount)?),
    DataStatus::Available,
    Some(source_at),
    observed_at,
    ProviderId::Sina,
    batch_id.clone(),
)
```

The batch provenance source time is the final point's source time. A request
with `date()` set returns explicit `Unsupported` before transport.

- [ ] **Step 4: Run all Sina tests**

Run:

```bash
cargo test -p magic-sina-rs --all-targets --offline
```

Expected: all tests pass.

- [ ] **Step 5: Commit minute support**

```bash
git add crates/magic-sina-rs/src/minute.rs crates/magic-sina-rs/src/bars.rs
git commit -m "feat: add Sina current minute data"
```

### Task 5: Add real live and bounded concurrent probes

**Files:**
- Create: `crates/magic-sina-rs/examples/live_probe.rs`
- Create: `crates/magic-sina-rs/examples/load_probe.rs`
- Create: `crates/magic-sina-rs/README.md`

- [ ] **Step 1: Add live probe output for every supported family**

Default `MAGIC_SINA_CODES` to
`600396.SH,000001.SZ,920118.BJ` and
`MAGIC_SINA_TIMEOUT_SECS` to `10`.

Print:

```text
provider=sina-web capabilities=...
quotes count=... provenance=... quality=...
quote code=... name=... price=... volume_lots=... amount_yuan=... source_at=...
order_books count=...
  level=1..5 bid_price=... bid_lots=... ask_price=... ask_lots=...
security_metadata count=...
bars interval=Minute1|Minute5|Minute15|Minute30|Hour1|Day ...
minute_current code=... count=...
unsupported trades=true money_flow=true auction=true fundamentals=true corporate_actions=true blocks=true
live_probe_status=passed
```

Fail the process on empty required families, cardinality mismatch, excessive
bar count or any transport/protocol error.

- [ ] **Step 2: Add a bounded load probe and tests**

Support `quotes`, `bars`, `minute`, `mixed`. Set:

```rust
const MAX_REQUESTS: usize = 40;
const MAX_CONCURRENCY: usize = 4;
```

Defaults are 20 requests and 4 threads. Clone one `SinaClient` so workers share
the pooled agent. Print provider, operation, requests, concurrency, successes,
failures, records, elapsed seconds, requests/second and microsecond
p50/p95/max. Exit nonzero if any request fails.

Unit tests must reject zero, concurrency above requests, requests above 40,
concurrency above 4 and unknown operations.

- [ ] **Step 3: Compile all targets**

Run:

```bash
cargo test -p magic-sina-rs --all-targets --locked --offline
```

Expected: library, contract and example tests pass.

- [ ] **Step 4: Run the real live probe**

Run:

```bash
RUSTUP_TOOLCHAIN=stable cargo run -p magic-sina-rs --example live_probe --release
```

Expected: real records for all three exchanges and `live_probe_status=passed`.

- [ ] **Step 5: Run the bounded mixed load probe**

Run:

```bash
RUSTUP_TOOLCHAIN=stable cargo run -p magic-sina-rs --example load_probe --release
```

Expected: 20 successes, zero failures, latency/throughput output and
`load_probe_status=passed`.

- [ ] **Step 6: Commit probes**

```bash
git add crates/magic-sina-rs
git commit -m "test: add Sina live and load probes"
```

### Task 6: Document and package Sina

**Files:**
- Create: `docs/integrations/sina-web.md`
- Modify: `README.md`
- Modify: `docs/DEPLOYMENT.md`
- Modify: `CHANGELOG.md`
- Modify: `tools/compliance/check.sh`
- Modify: `tools/release/package.sh`

- [ ] **Step 1: Write the integration manual**

Document:

- official endpoint URLs and required Referer;
- fields 0..32 and GB18030;
- share-to-lot conversion and CNY amounts;
- exact supported markets/periods/current-minute behavior;
- daily amount absence and partial metadata;
- all false capabilities with field-level reasons;
- environment variables and both probe commands;
- outbound domains, no local writes, no credentials/cookies;
- public-web/no-SLA production boundary and router registration.

- [ ] **Step 2: Update root capability and deployment truth**

Add Sina to the workspace table, provider capability matrix, quick commands,
platform/network table, package binary list and detailed-doc links. State only
the periods/markets proven by the live probe and label Sina supplemental.

Add `hq.sinajs.cn:443` and `quotes.sina.cn:443` to deployment egress and include
Sina live/load health checks.

- [ ] **Step 3: Update compliance**

Require `docs/integrations/sina-web.md` and
`crates/magic-sina-rs/Cargo.toml`. Update the exact workspace member assertion
to:

```text
members = ["crates/magic-market-core", "crates/magic-market-router", "crates/magic-tdx-rs", "crates/magic-emquant-rs", "crates/magic-tencent-rs", "crates/magic-sina-rs"]
```

Extend the router production-dependency denial regex to include `sina`.

- [ ] **Step 4: Package two Sina probes**

Build/install:

```text
bin/magic-sina-live-probe
bin/magic-sina-load-probe
```

using the same isolated target, host triple, locked/offline release build and
SHA-256 manifest path as the existing provider probes.

- [ ] **Step 5: Run docs/compliance/diff gates**

Run:

```bash
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
git diff --check
```

Expected: all commands pass.

- [ ] **Step 6: Commit documentation and packaging**

```bash
git add README.md CHANGELOG.md docs/DEPLOYMENT.md docs/integrations/sina-web.md tools/compliance/check.sh tools/release/package.sh
git commit -m "docs: document and package Sina provider"
```

### Task 7: Complete release verification and delivery

**Files:**
- Modify: `.planning/2026-07-23-sina-provider/task_plan.md`
- Modify: `.planning/2026-07-23-sina-provider/progress.md`

- [ ] **Step 1: Run formatting and complete deterministic gates**

Run:

```bash
cargo fmt --all
cargo check --workspace --all-targets --locked --offline
cargo test --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked --offline
cargo test --workspace --doc --locked --offline
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
git diff --check
```

Expected: every gate passes.

- [ ] **Step 2: Re-run live evidence after final source changes**

Run the Sina live probe and default 20/4 mixed load probe again. Record
timestamp, instruments, family counts, success/failure, throughput and
p50/p95/max in `progress.md`. Do not describe one run as an SLA.

- [ ] **Step 3: Re-run the newly entitled Choice/EMQuant live probe**

Run:

```bash
RUSTUP_TOOLCHAIN=stable cargo run -p magic-emquant-rs --example live_probe --release
```

Record whether Quote, bars, minute bars, order book and money flow pass. Keep
device activation distinct from product entitlement; any remaining vendor code
is reported exactly and does not change Sina capability truth.

- [ ] **Step 4: Perform completion review**

Review every changed file for secret/cookie leakage, unbounded input/response,
wrong share/lot units, missing source evidence, overstated capability,
presentation-HTML parsing, silent fallbacks, stale-data promotion and accidental
inclusion of the user's untracked requirements file.

- [ ] **Step 5: Commit final planning/evidence updates**

```bash
git add .planning/2026-07-23-sina-provider
git commit -m "chore: record Sina provider verification"
```

- [ ] **Step 6: Push and verify the remote commit**

Run:

```bash
git push origin main
git rev-parse HEAD
git ls-remote origin refs/heads/main
```

Expected: local HEAD equals remote `refs/heads/main`.

- [ ] **Step 7: Build and verify the exact final release package**

Run:

```bash
bash tools/release/package.sh
```

Verify `RELEASE_REVISION` equals final HEAD, exactly seven probe binaries are
present, every `SHA256SUMS` entry passes, Sina docs are included, and the package
contains no cookies, credentials, vendor runtime libraries or the user's
untracked requirements document.
