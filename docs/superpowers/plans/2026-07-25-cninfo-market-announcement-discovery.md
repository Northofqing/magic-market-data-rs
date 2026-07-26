# CNInfo Whole-Market Announcement Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a provider-neutral, source-evidenced whole-market announcement
operation backed by CNInfo's native market list.

**Architecture:** Core owns a request and Provider trait distinct from
instrument announcements. CNInfo owns strict native market pagination and
mapping. Router owns post-provider admission plus an explicit complete-empty
policy.

**Tech Stack:** Rust, serde/serde_json, existing CNInfo HTTPS transport,
provider-neutral Core records, failover Router, fixture-driven TDD.

---

### Task 1: Core contract

**Files:**
- Create: `crates/magic-market-core/src/market_announcements.rs`
- Modify: `crates/magic-market-core/src/lib.rs`
- Test: `crates/magic-market-core/tests/market_announcements.rs`

- [ ] Write a failing public-interface test constructing an inclusive
  `MarketAnnouncementRequest` and proving zero, over-300 and reversed ranges
  fail.
- [ ] Run
  `cargo test -p magic-market-core --test market_announcements --locked --offline`
  and verify the missing contract is RED.
- [ ] Add the validated request with `start()`, `end()`, `limit()` accessors,
  checked serde reconstruction, and `MarketAnnouncements`.
- [ ] Re-run the test and verify GREEN.

### Task 2: CNInfo native market Provider

**Files:**
- Create: `crates/magic-cninfo-rs/src/market_announcements.rs`
- Modify: `crates/magic-cninfo-rs/src/lib.rs`
- Test: `crates/magic-cninfo-rs/tests/market_announcements.rs`

- [ ] Write one failing public-interface fixture test that requires
  `stock=` empty, fixed page width, source-backed SH/SZ/BJ identities and
  record/batch evidence.
- [ ] Run
  `cargo test -p magic-cninfo-rs --test market_announcements --locked --offline`
  and verify the missing Provider implementation is RED.
- [ ] Implement one complete-page fetch, strict page metadata validation,
  source row mapping and strict batch evidence.
- [ ] Re-run the tracer test and verify GREEN.
- [ ] Add one failing test at a time for cross-page totals/order, exact row
  counts, equivalent/conflicting duplicates, configured page exhaustion and
  exact verified empty; implement only enough behavior for each GREEN cycle.

### Task 3: Router admission

**Files:**
- Create: `crates/magic-market-router/src/market_announcements.rs`
- Modify: `crates/magic-market-router/src/lib.rs`
- Modify: `crates/magic-market-router/src/router.rs`
- Test: `crates/magic-market-router/tests/market_announcements.rs`

- [ ] Write a failing test proving the market adapter validates limit, date
  range, source time, equity identity and unique IDs.
- [ ] Add `MarketAnnouncementRouter` and `market_announcement_source`.
- [ ] Write a failing route test proving default policy rejects an empty batch
  while `with_accept_complete_empty(true)` selects a strict complete empty
  batch.
- [ ] Add the default-off policy field and exact complete-empty admission.
- [ ] Re-run Router focused tests and verify GREEN.

### Task 4: Bounded production-trait probe

**Files:**
- Create: `crates/magic-cninfo-rs/examples/market_announcements_probe.rs`
- Test: `crates/magic-cninfo-rs/tests/unit/market_announcements_probe_tests.rs`

- [ ] Add a one-day, three-record probe that prints only normalized source
  identities, announcement IDs, provider times and batch evidence.
- [ ] Compile and run the probe against the real endpoint.
- [ ] Record non-empty admission or exact verified-empty evidence; otherwise
  leave capability unadmitted and report the typed failure.

### Task 5: Gates C and D

**Files:**
- Modify only documentation needed to report verified capability state.

- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run focused tests for the three affected crates.
- [ ] Run strict Clippy for the three affected crates with all targets.
- [ ] Run Rustdoc/doc checks and `bash tools/compliance/check.sh`.
- [ ] Run `git diff --check` and review the scoped diff for concurrent edits.
- [ ] Do not claim release readiness unless every gate and the bounded live
  probe pass.
