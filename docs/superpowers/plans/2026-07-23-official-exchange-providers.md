# Official Exchange Providers Implementation Plan

> **Execution rule:** implement in small test-first checkpoints, keep every
> capability false until its deterministic tests and a real HTTPS probe pass,
> and never add an insecure-TLS fallback.

**Goal:** Add official SSE, SZSE and HKEX read-only data sources behind the
existing provider-neutral Core and Router contracts.

**Architecture:** A new `magic-exchange-rs` crate owns three separately
configured clients (`SseClient`, `SzseClient`, `HkexClient`) but shares bounded
HTTP response, pacing, date-validation and JSON/JSONP helpers. Each client has
its own clone-shared request gate and host allowlist so one exchange cannot
silently redirect or fail over to another. Official records retain their
exchange provider identity and source timestamp; no public-web record is
relabeled as official.

**Constraints:** Rust 1.83.0, `#![forbid(unsafe_code)]`, HTTPS only, no account
or Cookie discovery, no fixture fallback in live probes, strict non-empty
complete batches, fixed remote page size plus local truncation, bounded
response size, zero redirects and at least one second between production
request starts.

---

## Task 1: Crate boundary and transport safety

**Files:**

- Create: `crates/magic-exchange-rs/Cargo.toml`
- Create: `crates/magic-exchange-rs/src/lib.rs`
- Create: `crates/magic-exchange-rs/src/transport.rs`
- Modify: `Cargo.toml`
- Modify: `tools/compliance/check.sh`

**Steps:**

1. Add failing tests for HTTPS-only exact host/path allowlists, redirects,
   content type, size/time bounds and clone-shared pacing.
2. Implement injectable transport request/response values and a Rustls
   production transport.
3. Add SSE/SZSE/HKEX provider identities to capability output without
   advertising any family.
4. Run crate tests and strict Clippy with Rust 1.83.

## Task 2: SSE official announcements

**Files:**

- Create: `crates/magic-exchange-rs/src/sse.rs`
- Create: `crates/magic-exchange-rs/tests/sse_announcements.rs`
- Create: `crates/magic-exchange-rs/fixtures/sse_announcements_page*.json`

**Steps:**

1. Fixture the official
   `queryCompanyBulletin.do` JSONP shape, `pageHelp` counters and PDF URLs.
2. Test Shanghai-only instrument validation, required source code/date,
   malformed JSONP, duplicate IDs, pagination overlap/gap, remote fixed page
   size and caller-limit truncation.
3. Implement `Announcements` with exact instrument/date-range filtering.
4. Add a live probe using 华电辽能 (`600396`) and require real non-empty records.

## Task 3: SZSE official announcements

**Files:**

- Create: `crates/magic-exchange-rs/src/szse.rs`
- Create: `crates/magic-exchange-rs/tests/szse_announcements.rs`
- Create: `crates/magic-exchange-rs/fixtures/szse_announcements_page*.json`

**Steps:**

1. Fixture the official POST `annList` JSON shape and its actual maximum page
   size of 50.
2. Test Shenzhen-only instrument validation, required source identity/date,
   exact date range, canonical detail/PDF URLs, pagination and local truncation.
3. Implement `Announcements`; reject partial pages and schema drift.
4. Add a live probe using a liquid Shenzhen equity and require real records.

## Task 4: Official announcement routing and operations

**Files:**

- Modify: `crates/magic-market-router/tests/intelligence_routing.rs`
- Create: `crates/magic-exchange-rs/examples/live_probe.rs`
- Create: `crates/magic-exchange-rs/examples/load_probe.rs`
- Create: `crates/magic-exchange-rs/README.md`
- Create: `docs/integrations/exchange-official.md`
- Modify: `README.md`
- Modify: `docs/DEPLOYMENT.md`
- Modify: `docs/PERFORMANCE_RESULTS.md`
- Modify: `.github/workflows/live-and-bench.yml`
- Modify: `tools/release/package.sh`

**Steps:**

1. Prove existing announcement router selects strict official batches and
   rejects identity/date/evidence mismatches.
2. Print every official announcement field and provenance in the live probe.
3. Add a serial, paced load probe with attempt-level—not falsely labeled
   transport-level—throughput and percentile/error metrics.
4. Document platform/network support and add the probes to CI/release.

## Task 5: SSE/SZSE official dragon-tiger

**Files:**

- Modify: `crates/magic-exchange-rs/src/sse.rs`
- Modify: `crates/magic-exchange-rs/src/szse.rs`
- Add: exchange-specific fixtures/tests and probe coverage

**Steps:**

1. Model official list and seat-detail responses using existing
   `DragonTigerEntry`/`DragonTigerSeat` contracts.
2. Require exact source instrument and trading date on every row; parse SSE
   text state explicitly rather than by column guessing.
3. Preserve official buy/sell rank and amount units; reject incomplete top-five
   pages instead of synthesizing seats.
4. Advertise and document only after both deterministic and real probes pass.

## Task 6: SZSE Quote and order book

**Files:**

- Modify: `crates/magic-exchange-rs/src/szse.rs`
- Add: quote/order-book fixtures, tests and probe coverage

**Steps:**

1. Map the verified `getTimeData` response to Core Quote/OrderBook contracts.
2. Convert source lots to shares exactly once and test the factor.
3. Require source market time, instrument identity, ordered book levels and
   non-crossed prices.
4. Keep SSE Quote unsupported while its public host requires obsolete TLS.

## Task 7: HKEX northbound daily statistics

**Files:**

- Modify: `crates/magic-market-core/src/capital.rs`
- Modify: `crates/magic-market-router/src/adapters.rs`
- Create: `crates/magic-exchange-rs/src/hkex.rs`
- Add: Core/Router/provider fixtures and tests

**Steps:**

1. Add a lossless daily northbound contract for turnover, trade count, ETF
   turnover and Top10 records; represent source sentinels explicitly.
2. Do not call post-2024 turnover “net inflow”; preserve the source metric name.
3. Parse the official static DailyStat JavaScript with exact trading date and
   channel identity.
4. Add live/load probes and capability gates after real acceptance.

## Task 8: Release gate

1. Run format, workspace check/test, strict Clippy, rustdoc/doctest, docs links,
   compliance and shell syntax with Rust 1.83 and `--locked --offline`.
2. Run every new real probe and save bounded performance evidence.
3. Run isolated release preflight/package verification.
4. Commit only tracked project inputs, leave unrelated user files unstaged,
   and push the verified checkpoint.
