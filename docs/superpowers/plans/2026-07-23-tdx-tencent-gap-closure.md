# TDX and Tencent Gap Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close every currently verifiable TDX/Tencent capability gap and turn every unverifiable family into a field-specific tested `Unsupported` result.

**Architecture:** Keep TDX on its audited binary protocol while adding evidence-gated Beijing/time handling. Split Tencent by native HTTPS data family, add provider-neutral minute contracts to Core, and require strict fixtures plus real probes before advertising a capability.

**Tech Stack:** Rust 2021/no fixed MSRV, `ureq`, `serde_json`, `encoding_rs`, existing `magic-market-core` contracts, TDX binary protocol, Tencent HTTPS endpoints.

---

### Task 1: Freeze live protocol evidence and RED contracts

**Files:**
- Modify: `crates/magic-tdx-rs/examples/live_probe.rs`
- Modify: `crates/magic-tencent-rs/examples/live_probe.rs`
- Modify: `crates/magic-tdx-rs/tests/capabilities.rs`
- Create: `crates/magic-tencent-rs/tests/capabilities.rs`

- [ ] Add probes for TDX market candidates `0`, `1`, `2` using one current Beijing code and print returned market/code/raw quote-time values.
- [ ] Add Tencent live calls for `bj920118`, daily K line, current minute, previous-session minute, and current trades.
- [ ] Add failing capability tests requiring Tencent Beijing Quote/OrderBook, Bars, Minute, Trades, and SecurityMetadata.
- [ ] Add failing Core compile-contract tests for `MinutePoint`, `MinuteDataRequest`, and `MinuteData`.
- [ ] Run the focused tests and record the expected missing-type/false-capability failures.

### Task 2: Add the provider-neutral minute contract

**Files:**
- Modify: `crates/magic-market-core/src/provider.rs`
- Modify: `crates/magic-market-core/src/lib.rs`
- Modify: `crates/magic-market-core/tests/provider.rs`
- Modify: `crates/magic-market-core/tests/serde_contracts.rs`

- [ ] Define checked `MinutePoint` fields: instrument, minute timestamp, price, cumulative quantity, optional cumulative amount, source/observation/provider/batch evidence, and `DataStatus`.
- [ ] Define `MinuteDataRequest` with one instrument and optional strict ISO date.
- [ ] Define synchronous `MinuteData` and asynchronous `AsyncMinuteData` traits.
- [ ] Add serde round-trip and constructor-bypass tests for time, negative/non-finite cumulative values, missing evidence, and contradictory `Available` status.
- [ ] Run `cargo test -p magic-market-core --all-targets --locked --offline` and require all tests to pass.

### Task 3: Verify and implement TDX Beijing support

**Files:**
- Modify: `crates/magic-tdx-rs/src/adapter.rs`
- Modify: `crates/magic-tdx-rs/tests/adapter.rs`
- Modify: `crates/magic-tdx-rs/examples/live_probe.rs`

- [ ] Capture the candidate probe result and require exact returned market/code/cardinality instead of accepting a merely non-empty packet.
- [ ] If and only if candidate `0` uniquely returns the requested Beijing record, map `Exchange::Beijing` to `0` while retaining exchange identity in normalized records.
- [ ] Add unit tests proving Beijing uses the verified source market and Shanghai/Shenzhen mappings do not change.
- [ ] Run the real probe for Beijing Quote, daily bars, current minute, current trades, order book and metadata; any family rejected by the server remains explicitly unsupported in its entry point.

### Task 4: Add TDX normalized minute data and explicit unsupported dispositions

**Files:**
- Modify: `crates/magic-tdx-rs/src/adapter.rs`
- Modify: `crates/magic-tdx-rs/tests/adapter.rs`
- Modify: `crates/magic-tdx-rs/tests/capabilities.rs`
- Modify: `docs/TDX_CAPABILITIES.md`

- [ ] Implement `MinuteData` for blocking and smart clients using current and dated TDX minute operations.
- [ ] Normalize `HH:MM` using the requested/current China date, preserve cumulative source quantities, and keep amount unavailable where TDX does not provide it.
- [ ] Validate 240-point maximum, ordering, duplicate time, price, cumulative quantity, and exact requested date.
- [ ] Add field-specific assertions that TDX money flow and auction return `Unsupported` because their required normalized fields are absent.
- [ ] Keep `source_at` empty unless the raw-time experiment proves a complete decoder; document the raw value and rejected inference.

### Task 5: Split and harden Tencent transport/symbol/snapshot modules

**Files:**
- Modify: `crates/magic-tencent-rs/src/lib.rs`
- Create: `crates/magic-tencent-rs/src/client.rs`
- Create: `crates/magic-tencent-rs/src/transport.rs`
- Create: `crates/magic-tencent-rs/src/symbol.rs`
- Create: `crates/magic-tencent-rs/src/snapshot.rs`
- Create: `crates/magic-tencent-rs/src/time.rs`

- [ ] Move existing behavior without changing the public client/error surface.
- [ ] Generalize bounded transport for GBK snapshot and JSON/text data-family response limits while retaining HTTPS-only and redirect refusal.
- [ ] Add Beijing `bj` encoding and require response market code `62`; retain exact duplicate/order/cardinality checks.
- [ ] Implement partial `SecurityMetadataProvider` from direct snapshot name/ST evidence and explicit missing-field quality issues.
- [ ] Run existing tests after each move to prove no Shanghai/Shenzhen Quote or book regression.

### Task 6: Implement Tencent historical bars

**Files:**
- Create: `crates/magic-tencent-rs/src/bars.rs`
- Create: `crates/magic-tencent-rs/tests/bars.rs`
- Modify: `crates/magic-tencent-rs/src/client.rs`
- Modify: `crates/magic-tencent-rs/Cargo.toml`

- [ ] Add strict JSON envelope structs and reject nonzero response codes, missing symbol keys, ambiguous adjusted/unadjusted arrays, excessive counts, duplicates and trailing semantic fields.
- [ ] Map only live-verified day/week/month periods and supported adjustment arrays; return `Unsupported` for minute/hour/year or unverified adjustment combinations.
- [ ] Validate date, OHLC, positive prices, nonnegative source-lot volume and chronological ordering.
- [ ] Implement `HistoricalBars` with exact limit behavior and record/batch evidence.
- [ ] Add success fixtures plus malformed JSON, wrong symbol, bad OHLC, invalid date, excessive count and unsupported-period tests.

### Task 7: Implement Tencent current and historical minute data

**Files:**
- Create: `crates/magic-tencent-rs/src/minute.rs`
- Create: `crates/magic-tencent-rs/tests/minute.rs`
- Modify: `crates/magic-tencent-rs/src/client.rs`

- [ ] Parse current `minute/query` and dated `day/query` JSON shapes without treating their different optional amount fields as identical.
- [ ] Enforce 09:30–11:30/13:00–15:00 minute bounds, chronological uniqueness, at most 240 records, and monotonic cumulative quantity/amount.
- [ ] Require the response date to equal a requested historical date; reject ambiguous missing dates.
- [ ] Implement `MinuteData` and preserve missing Beijing cumulative amount as unavailable rather than zero.
- [ ] Add current/historical, lunch-boundary, regression, duplicate, date-mismatch and truncated-field tests.

### Task 8: Implement Tencent current-session trades with strict paging

**Files:**
- Create: `crates/magic-tencent-rs/src/trades.rs`
- Create: `crates/magic-tencent-rs/tests/trades.rs`
- Modify: `crates/magic-tencent-rs/src/client.rs`

- [ ] Parse the JavaScript wrapper and every slash/pipe-delimited row with exact field count and exact `B/S/M` direction mapping.
- [ ] Add bounded page size/page count, stable sequence checks, duplicate rejection and all-or-error pagination up to `TradesRequest::limit`.
- [ ] Validate session time, positive price, nonnegative source-lot quantity/amount and price-volume-amount order of magnitude.
- [ ] Implement `Trades` for current session and return explicit `Unsupported` for dated requests because the endpoint exposes no verified date selector.
- [ ] Add multipage, malformed wrapper, sequence gap, duplicate, unknown side, excessive page and historical-request tests.

### Task 9: Extend live and bounded-load acceptance

**Files:**
- Modify: `crates/magic-tencent-rs/examples/live_probe.rs`
- Modify: `crates/magic-tencent-rs/examples/load_probe.rs`
- Modify: `crates/magic-tdx-rs/examples/live_probe.rs`
- Modify: `docs/PERFORMANCE_RESULTS.md`

- [ ] Print every new batch, record field, evidence timestamp, status and quality issue for Shanghai, Shenzhen and Beijing samples.
- [ ] Make the live probe fail on wrong cardinality, symbol/date mismatch, empty required records or any silently partial page.
- [ ] Extend the bounded probe with an operation selector and retain hard caps of 100 total requests and 8 workers.
- [ ] Run release probes during a known market phase and record request count, success count, records, p50/p95/max latency without claiming an SLA.

### Task 10: Documentation, compatibility and release gates

**Files:**
- Modify: `crates/magic-tencent-rs/README.md`
- Modify: `crates/magic-tdx-rs/README.md`
- Modify: `docs/integrations/tencent-web.md`
- Modify: `docs/TDX_CAPABILITIES.md`
- Modify: `README.md`
- Modify: `CHANGELOG.md`

- [ ] Update capability matrices with implemented periods/markets and field-specific unsupported reasons.
- [ ] Document units, pagination, endpoint stability, authorization/SLA limitations and deployment network requirements.
- [ ] Run `cargo fmt --all --check`.
- [ ] Run `cargo test --workspace --all-targets --locked --offline`.
- [ ] Run strict workspace Clippy, Rust stable all-target check, rustdoc/doctest, link and compliance scripts.
- [ ] Run both real release probes and bounded load probes.
- [ ] Review the final diff, ensure the user's untracked requirements document is not staged, commit scoped files, and push `main`.
