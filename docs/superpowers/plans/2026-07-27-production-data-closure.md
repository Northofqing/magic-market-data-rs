# Production Data Closure Implementation Plan

> Execute this plan on `feat/production-data-closure` in
> `.worktrees/production-data-closure`. Preserve explicit failures and provenance;
> do not add downstream path dependencies.

**Goal:** Implement and verify the remaining public-market production contracts
for security lifecycle, rankings/breadth/consensus, CFFEX delivery admission, and
strict realtime freshness while keeping Level-2 and broker-account boundaries
honest and usable.

**Architecture:** Add provider-neutral checked contracts in Core, normalize only
source-proven fields in individual provider crates, enforce evidence and freshness
in Router, and keep application composition outside provider-neutral crates.
Each public capability receives deterministic tests and a bounded live operation;
licensed/account-only capabilities receive conformance seams and remain
unadvertised without credentials and evidence.

**Tech stack:** Rust workspace, Serde, thiserror, ureq/explicit HTTPS transports,
existing `DataBatch`/`SourceEvidence`/Router abstractions, Cargo release gates.

---

## Task 1: Register the remaining business rules

**Files:**

- Modify: `docs/business_rules.md`
- Modify: `docs/requirements_traceability.md`
- Test: `tools/compliance/check.sh`

1. Add a failing compliance assertion for rule identifiers BR-033 through BR-035.
2. Run `bash tools/compliance/check.sh` and confirm the missing rules fail.
3. Register:
   - BR-033: strict source-time freshness never substitutes observed time;
   - BR-034: full-market ranking/breadth coverage, units, code+name, and skew;
   - BR-035: licensed auction and authenticated account boundaries.
4. Map each rule to its Core/provider/Router/tests/live evidence.
5. Re-run compliance and commit the documentation gate.

## Task 2: Add provider-neutral corporate-action contracts

**Files:**

- Create: `crates/magic-market-core/src/lifecycle.rs`
- Modify: `crates/magic-market-core/src/lib.rs`
- Modify: `crates/magic-market-core/src/provider.rs`
- Create: `crates/magic-market-core/tests/lifecycle.rs`
- Modify: `crates/magic-market-core/tests/serde_contracts.rs`

1. Write failing tests for valid distribution/split/reverse-split actions,
   invalid dates, invalid terms, non-finite values, evidence mismatch, and serde.
2. Write a failing test proving `SecurityMetadata` satisfies `SourcedRecord`.
3. Implement checked `CorporateActionCategory`, `CorporateActionStatus`,
   `CorporateActionTerms`, `CorporateAction`, `CorporateActionRequest`, and
   `CorporateActions`.
4. Implement `SourcedRecord` for `CorporateAction` and `SecurityMetadata`.
5. Run:

   ```bash
   cargo test -p magic-market-core --test lifecycle --test serde_contracts --locked --offline
   ```

6. Commit as `feat(core): add security lifecycle contracts`.

## Task 3: Harden TDX finance and XDXR parsing

**Files:**

- Modify: `crates/magic-tdx-rs/src/protocol/parsers.rs`
- Modify: `crates/magic-tdx-rs/src/protocol/types.rs`
- Modify: TDX parser tests in `crates/magic-tdx-rs/src/protocol/parsers.rs`

1. Add failing fixtures for finance response market/code mismatch.
2. Add failing fixtures for malformed/future IPO dates.
3. Add failing XDXR fixtures for a declared-count truncated record, invalid date,
   NaN/Inf values, and trailing/inconsistent bytes.
4. Parse and validate response identity rather than copying request identity.
5. Consume exactly the declared XDXR count; never `break` into partial success.
6. Run:

   ```bash
   cargo test -p magic-tdx-rs protocol::parsers --locked --offline
   ```

7. Commit as `fix(tdx): make finance and xdxr parsing atomic`.

## Task 4: Implement normalized TDX lifecycle providers

**Files:**

- Modify: `crates/magic-tdx-rs/src/adapter.rs`
- Modify: `crates/magic-tdx-rs/src/service/finance.rs`
- Modify: `crates/magic-tdx-rs/src/service/mod.rs`
- Modify: `crates/magic-tdx-rs/examples/live_probe.rs`
- Create: `crates/magic-tdx-rs/tests/lifecycle_provider.rs`

1. Add failing provider tests for exact listing date, request identity, Beijing
   `Unsupported`, complete empty actions, date filtering, ordering, duplicates,
   unknown categories, and shared batch evidence.
2. Enrich TDX `SecurityMetadata` from verified finance `ipo_date`.
3. Implement `CorporateActions` for the blocking TDX provider using source
   categories 1, 11, and 12 only.
4. Keep derived board/rule fields incomplete and add a precise quality issue.
5. Extend the live probe with exact-value listing and corporate-action assertions
   plus one verified-empty request.
6. Run:

   ```bash
   cargo test -p magic-tdx-rs --test lifecycle_provider --locked --offline
   cargo test -p magic-tdx-rs adapter::tests --locked --offline
   ```

7. Commit as `feat(tdx): normalize listing and corporate actions`.

## Task 5: Route lifecycle evidence

**Files:**

- Modify: `crates/magic-market-router/src/adapters.rs`
- Modify: `crates/magic-market-router/src/lib.rs`
- Create: `crates/magic-market-router/tests/lifecycle_routing.rs`

1. Add failing tests proving real `SecurityMetadataRouter::route` compilation and
   provider/batch mismatch rejection.
2. Add failing corporate-action routing tests for fallback, invalid request stop,
   verified empty, range/order/duplicate rejection, and exact evidence.
3. Add `CorporateActionRouter` and `corporate_action_source`.
4. Run:

   ```bash
   cargo test -p magic-market-router --test lifecycle_routing --locked --offline
   ```

5. Commit as `feat(router): route security lifecycle evidence`.

## Task 6: Add typed ranking and breadth contracts

**Files:**

- Modify: `crates/magic-market-core/src/signals.rs`
- Modify: `crates/magic-market-core/src/lib.rs`
- Create: `crates/magic-market-core/tests/market_rankings.rs`

1. Add failing tests for `VolumeRatio` and `MainNetInflow` metric/unit validation,
   code+name requirements, source session/date, continuous unique ranks, and serde.
2. Add failing breadth tests for:
   `valid == up + down + flat`, limit subsets, coverage bounds, source skew,
   universe identity, and evidence.
3. Extend ranking contracts without encoding breadth as a synthetic rank.
4. Add `MarketBreadthSnapshot`, request type, and `MarketBreadth` trait.
5. Run:

   ```bash
   cargo test -p magic-market-core --test market_rankings --locked --offline
   ```

6. Commit as `feat(core): type rankings and market breadth`.

## Task 7: Implement Eastmoney full-market rankings

**Files:**

- Create: `crates/magic-eastmoney-rs/src/market_rankings.rs`
- Modify: `crates/magic-eastmoney-rs/src/lib.rs`
- Modify: `crates/magic-eastmoney-rs/src/transport.rs`
- Modify: `crates/magic-eastmoney-rs/examples/live_probe.rs`
- Create: `crates/magic-eastmoney-rs/tests/market_rankings.rs`

1. Probe the existing allowlisted `push2.eastmoney.com/api/qt/clist/get` schema for
   volume ratio and main net inflow; record exact fields and source-time behavior.
2. Add failing fixtures for total/page contradictions, duplicate instruments,
   missing code/name, mixed timestamps, wrong market, non-finite metrics,
   incomplete requested limit, and wrong ordering.
3. Implement independent `MarketRankings` operations per metric. Advertise only
   metrics that pass the source-time and full-market coverage gates.
4. Print code and name together in the live probe.
5. Add the current-day strict post-close operation to the live probe.
6. Run deterministic tests and then bounded live probes for each operation.
7. Commit as `feat(eastmoney): add admitted market rankings`.

## Task 8: Implement market breadth and concept projection

**Files:**

- Create: `crates/magic-market-analysis/src/breadth.rs`
- Modify: `crates/magic-market-analysis/src/lib.rs`
- Create: `crates/magic-market-analysis/tests/breadth.rs`
- Modify: `crates/magic-tdx-rs/src/service/blocks.rs`
- Create: `crates/magic-tdx-rs/tests/concept_hits.rs`

1. Add failing breadth tests for coverage, skew, duplicate instruments, halted or
   missing prices, limit-up/down subsets, and evidence preservation.
2. Implement local breadth analysis over an explicitly identified quote universe;
   mark provider as `LocalAnalysis` and retain input batch evidence references.
3. Add failing concept projection tests for concept-only output, no duplicates,
   exact request coverage, verified empty, file-hash version, and Beijing
   `Unsupported`.
4. Implement `ConceptHits` for the proven TDX Shanghai/Shenzhen block snapshot.
5. Run:

   ```bash
   cargo test -p magic-market-analysis --test breadth --locked --offline
   cargo test -p magic-tdx-rs --test concept_hits --locked --offline
   ```

6. Commit as `feat: add breadth analysis and concept hits`.

## Task 9: Wire THS consensus and finish target-price aggregation

**Files:**

- Create: `crates/magic-market-router/examples/consensus_live.rs`
- Modify: `crates/magic-market-router/Cargo.toml` dev-dependencies only
- Modify: `crates/magic-eastmoney-rs/src/reports.rs`
- Modify: `crates/magic-eastmoney-rs/examples/live_probe.rs`
- Create: `crates/magic-eastmoney-rs/tests/target_price.rs`

1. Add an example-only THS dev-dependency; do not add a normal Router dependency.
2. Register `consensus_source` in the example and prove selection, verified empty,
   and failure classification.
3. Run the current THS live admission and retain the command/result artifact.
4. Add failing target-price tests for contributor count, observation period,
   min/mean/max consistency, missing source date, duplicate institution/report,
   pagination completeness, and evidence.
5. Implement complete Eastmoney target-price aggregation or keep the capability
   false with an exact diagnostic if the live source cannot prove those fields.
6. Commit as `feat: wire consensus and target price evidence`.

## Task 10: Enforce strict Router source freshness

**Files:**

- Modify: `crates/magic-market-core/src/probe.rs`
- Modify: `crates/magic-market-router/src/router.rs`
- Modify: `crates/magic-market-router/src/adapters.rs`
- Modify: `crates/magic-market-router/examples/live_probe.rs`
- Create: `crates/magic-market-router/tests/freshness.rs`

1. Add failing tests for exactly 5 seconds, 6 seconds, future time, malformed time,
   missing record time, record/batch mismatch, oldest record, milliseconds,
   timezone offsets, and no-freshness policies.
2. Expose a reusable Core source-time parser/validator without weakening probe
   admission.
3. Add optional `max_source_age` to `AcceptancePolicy`.
4. Add quote-family record timestamp validation before source selection.
5. Update the live example to print selected provider and measured source age.
6. Run:

   ```bash
   cargo test -p magic-market-router --test freshness --locked --offline
   cargo test -p magic-market-router --test router --locked --offline
   ```

7. Commit as `feat(router): enforce source timestamp freshness`.

## Task 11: Remediate CFFEX transport and formal admission

**Files:**

- Modify: `crates/magic-exchange-rs/Cargo.toml`
- Modify: `crates/magic-exchange-rs/src/transport.rs`
- Modify: `crates/magic-exchange-rs/src/cffex.rs`
- Modify: `crates/magic-exchange-rs/examples/live_probe.rs`
- Create: `crates/magic-exchange-rs/tests/cffex_transport.rs`
- Modify: `docs/integrations/exchange-official.md`

1. Add transport conformance tests for allowlist, redirect, MIME, maximum body,
   timeout, pacing, TLS error classification, and explicit backend selection.
2. Add a second explicitly selected TLS backend only if it can be kept within the
   same contracts and the dependency policy passes.
3. Refactor probe and formal trait to one bounded internal operation.
4. Run current/recent-month live probes from each configured backend.
5. Only after the exact IF/IH/IC/IM official result passes admission, set the
   capability true. Otherwise retain false and archive the exact transport failure.
6. Commit as `fix(exchange): make cffex delivery admission executable`.

## Task 12: Land licensed/account conformance boundaries

**Files:**

- Create: `crates/magic-market-core/src/conformance.rs`
- Modify: `crates/magic-market-core/src/lib.rs`
- Create: `crates/magic-market-core/tests/auction_conformance.rs`
- Create: `docs/integrations/level2-auction.md`
- Create: `docs/integrations/broker-account-boundary.md`

1. Write failing conformance tests for complete auction fields, exact instrument
   identity, provider source time, batch evidence, and missing unmatched queues.
2. Add a reusable public conformance verifier for authorized `Auctions`
   implementations without introducing a concrete licensed dependency.
3. Document environment/credential injection and the narrower diagnostic boundary.
4. Document that broker account data belongs to a separate authenticated gateway;
   prohibit browser-cookie scraping and downstream path dependencies.
5. Run Core tests and compliance.
6. Commit as `docs: define licensed market data boundaries`.

## Task 13: Documentation, live evidence, and release audit

**Files:**

- Modify: `README.md`
- Modify: relevant `crates/*/README.md`
- Modify: `README.md` provider capability matrix
- Create/update: bounded live-admission artifacts under the established evidence path

1. Update capability tables only from current implementation and live evidence.
2. Show ranked stocks as `code + name`; document coverage, metric unit, source
   date/session, skew, and freshness policy.
3. Document explicit remaining `Unsupported` states and why they cannot be
   satisfied by public crawling.
4. Run the full release audit:

   ```bash
   cargo fmt --all -- --check
   cargo check --workspace --all-targets --locked --offline
   cargo test --workspace --all-targets --locked --offline
   cargo clippy --workspace --all-targets --locked --offline -- -D warnings
   cargo doc --workspace --no-deps --locked --offline
   bash tools/compliance/check.sh
   bash tools/docs/check_links.sh
   cargo llvm-cov --workspace --all-features --locked --offline --summary-only
   bash tools/release/package.sh
   ```

5. Run a requirement-by-requirement completion audit against the design and this
   plan. Do not treat inaccessible live endpoints as admitted.
6. Commit as `docs: publish production data closure evidence`.
