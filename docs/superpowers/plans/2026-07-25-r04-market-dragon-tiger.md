# R-04 Market Dragon-Tiger Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use TDD and execute each
> checkbox as one RED/GREEN step. This task is executed inline because the
> parent explicitly assigned an independent parallel slice.

**Goal:** Add atomic whole-market dragon-tiger discovery with complete
per-entry seat details for R-04.

**Architecture:** Core owns a small market-disclosure interface and validates
the atomic entry/seat invariant. The Eastmoney Adapter owns full-day discovery,
`TRADE_ID` identity, stable dedup/sort/limit and exact per-entry seat requests.
The Router validates normalized batch evidence without source parsing.

**Tech Stack:** Rust, `magic-market-core`, `magic-eastmoney-rs`,
`magic-market-router`, injected fixture transports, real Eastmoney HTTPS probe.

---

### Task 1: Register contract and Core types

**Files:**
- Modify: `docs/business_rules.md`
- Modify: `crates/magic-market-core/src/signals.rs`
- Modify: `crates/magic-market-core/src/lib.rs`
- Test: `crates/magic-market-core/tests/signals.rs`

- [x] Write a failing public-interface test constructing one disclosure with
  two incomplete side groups and assert explicit rejection.
- [x] Run `cargo test -p magic-market-core --test signals
  market_dragon_tiger_disclosure_requires_exact_buy_five_sell_five`.
- [x] Add `MarketDragonTigerRequest`, `DragonTigerDisclosure`, constructor
  validation, getters, `SourcedRecord`, and `MarketDragonTigerData`.
- [x] Re-run the focused test and all `magic-market-core` tests.

### Task 2: Implement Eastmoney full-market discovery

**Files:**
- Modify: `crates/magic-eastmoney-rs/src/dragon_tiger.rs`
- Test: `crates/magic-eastmoney-rs/tests/unit/dragon_tiger_tests.rs`

- [x] Add one failing test proving two rows for one security/date with distinct
  `TRADE_ID` values both survive.
- [x] Implement source identity parsing as
  `<code>:<YYYY-MM-DD>:<TRADE_ID>`.
- [x] Add one failing test proving identical duplicates collapse but a
  conflicting duplicate fails.
- [x] Implement deterministic identity-map admission.
- [x] Add one failing test proving net-descending deterministic order and
  post-dedup limit.
- [x] Implement complete bounded day fetch, local stable sorting, then limit.

### Task 3: Fetch exact seat groups atomically

**Files:**
- Modify: `crates/magic-eastmoney-rs/src/dragon_tiger.rs`
- Test: `crates/magic-eastmoney-rs/tests/unit/dragon_tiger_tests.rs`

- [x] Add a failing transport test requiring both seat URLs to contain the
  selected `TRADE_ID`.
- [x] Implement exact entry filtering and shared batch evidence.
- [x] Add failing tests for missing/extra/mismatched seat rows.
- [x] Construct `DragonTigerDisclosure` only after a complete 5+5 group exists.
- [x] Run all `magic-eastmoney-rs` tests.

### Task 4: Add Router adapter

**Files:**
- Modify: `crates/magic-market-router/src/adapters.rs`
- Modify: `crates/magic-market-router/src/lib.rs`
- Test: `crates/magic-market-router/tests/intelligence_routing.rs`

- [x] Add a failing adapter test for canonical order rejection; date, result
  count, identity and evidence validation are enforced in the adapter.
- [x] Add `MarketDragonTigerRouter` and `market_dragon_tiger_source`.
- [x] Run focused and full intelligence Router tests.

### Task 5: Live probe and gates

**Files:**
- Modify: `crates/magic-eastmoney-rs/examples/live_probe.rs`
- Modify: `docs/integrations/eastmoney-web.md`

- [x] Add a probe section controlled by
  `MAGIC_EASTMONEY_DRAGON_TIGER_DATE`, printing only normalized identities and
  counts.
- [x] Run the real probe for a known trading date and capture batch provider,
  source date, entry IDs, and 10-seat cardinality.
- [ ] Run `cargo fmt --all -- --check`.
- [x] Run target crate tests and Clippy with warnings denied.
- [x] Run compliance and documentation checks.
- [ ] Run workspace formatting/tests, recording unrelated failures separately.
