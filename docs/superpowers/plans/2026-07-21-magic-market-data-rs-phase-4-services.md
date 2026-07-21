# Phase 4: Complete Services, Stable Facade, and Core Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose every pinned-upstream pure-Rust data operation through strict typed services, wire the actual capability set onto the four clients, and provide checked conversion into `magic-market-core` provider results.

**Architecture:** Capability services own request validation, packet selection, pagination/chunking, parsing, adjustment, empty/no-data interpretation, and provenance assembly; clients only supply their execution strategy. Source APIs return typed batches such as `DataBatch<SecurityBar>` and `DataBatch<SecurityQuote>`, while `TdxProvider` applies explicit checked normalization and quality validation before implementing provider-neutral traits.

**Tech Stack:** Rust 1.83, the Phase 1 core contracts, Phase 2 strict protocol/readers, Phase 3 client executors, Serde, tracing, compile-time capability tests, scripted loopback integration.

---

## Exit gate

Phase 4 is complete only when every pinned-upstream Rust-callable operation has a tested facade disposition, no v1 pure-Rust capability is silently absent, strict multi-page/multi-chunk/adjusted operations are atomic, all returned batches contain provenance, and core normalization can never substitute missing values or source timestamps.

### Task 1: Create shared strict service execution contracts

**Files:**
- Create: `crates/magic-tdx-rs/src/service/mod.rs`
- Create: `crates/magic-tdx-rs/src/service/common.rs`
- Create: `crates/magic-tdx-rs/tests/services.rs`
- Modify: `crates/magic-tdx-rs/src/lib.rs`

- [ ] **Step 1: Write service-level empty/no-data/page tests**

Use a scripted in-memory executor and assert: an explicit protocol no-data code becomes `ErrorKind::NoData`; a successful frame with no records becomes `EmptyResponse`; a middle-page transport error becomes `IncompletePage`; duplicate/missing page ranges fail strict assembly; and best-effort assembly is possible only through an explicitly named method returning `BatchCompleteness::Partial`.

- [ ] **Step 2: Run and verify service contracts are absent**

Run: `cargo test -p magic-tdx-rs --test services common_`

Expected: compile failure.

- [ ] **Step 3: Define internal sync/async executor traits**

`BlockingExecutor` accepts `RequestEnvelope` and returns `ResponseEnvelope`. `AsyncExecutor` returns a `Send` future tied to `&self` without adding an async-trait dependency. Implement these private traits for Blocking, Direct, Async, and Smart according to their execution models; do not make raw packet execution public.

- [ ] **Step 4: Implement strict page/chunk assembly**

`StrictCollector<T>` records requested ranges, received ranges, page endpoint/attempt/elapsed, records, and errors. `finish_strict` rejects gaps, overlaps, out-of-order page identities, count mismatches, or any error and returns `DataBatch<T>` with complete provenance. `finish_best_effort` requires the caller's explicitly named API and returns partial provenance plus structured `PageOutcome` values.

- [ ] **Step 5: Implement result/provenance helpers**

Build request id, trace id, endpoint, fetched time, optional source time, requested/received count, page/chunk count, adjustment label, cache state, and completeness from actual execution. No helper may set `source_at` from local completion time. Empty and no-data classification happens before batch construction.

- [ ] **Step 6: Run and commit shared service contracts**

```bash
cargo test -p magic-tdx-rs --test services common_
cargo test -p magic-tdx-rs service::common
```

Expected: all pass.

```bash
git add crates/magic-tdx-rs/src/service crates/magic-tdx-rs/src/lib.rs crates/magic-tdx-rs/tests/services.rs
git diff --cached --check
git commit -m "feat(tdx): add strict service execution contracts"
```

### Task 2: Implement bars/index bars and atomic adjustment services

**Files:**
- Create: `crates/magic-tdx-rs/src/service/bars.rs`
- Modify: `crates/magic-tdx-rs/src/client/blocking.rs`
- Modify: `crates/magic-tdx-rs/src/client/direct.rs`
- Modify: `crates/magic-tdx-rs/src/client/asynchronous.rs`
- Modify: `crates/magic-tdx-rs/src/client/smart.rs`
- Create: `crates/magic-tdx-rs/tests/bars_service.rs`

- [ ] **Step 1: Write strict bars tests for every client strategy**

For source bars and index bars, exercise one page, all-pages pagination, exact category/start/count mapping, no data, empty success retry exhaustion, page 2 failure, page overlap, and front/back/no adjustment. Run equivalent scripted fixtures through Blocking, Direct, and Async. For Smart, verify bars retry/failover under its one operation budget.

- [ ] **Step 2: Run and verify client market methods are absent**

Run: `cargo test -p magic-tdx-rs --test bars_service`

Expected: compile failure.

- [ ] **Step 3: Implement source bar request services**

Map checked core `BarRequest` plus typed TDX category/market information into packet pages. Enforce configured page/record limits and strict range completeness. Return `DataBatch<SecurityBar>` or `DataBatch<IndexBar>` with source time derived only from valid encoded bar timestamps.

- [ ] **Step 4: Integrate atomic adjustment**

For `Adjustment::None`, parse and return without XDXR calls. For front/back adjustment, retrieve every required XDXR/context page through the same executor and deadline, apply Phase 2 factors, and return success only after all context validates. Preserve raw unadjusted source bars only in an explicitly separate advanced method; never return them for a failed adjusted request.

- [ ] **Step 5: Add typed methods to actual client capabilities**

Expose synchronous `bars`, `bars_all`, `index_bars`, and `index_bars_all` on Blocking and Direct; async equivalents on Async; and `bars`/`bars_all` on Smart. Method docs state page ordering, empty/no-data semantics, adjustment behavior, source-time meaning, and strategy-specific retry/queue behavior.

- [ ] **Step 6: Run and commit bars services**

```bash
cargo test -p magic-tdx-rs --test bars_service
cargo test -p magic-tdx-rs --test strict_failures adjustment_
```

Expected: all pass, including regression cases for ignored XDXR errors.

```bash
git add crates/magic-tdx-rs/src/service/bars.rs crates/magic-tdx-rs/src/client crates/magic-tdx-rs/tests/bars_service.rs
git diff --cached --check
git commit -m "feat(tdx): add strict bar services"
```

### Task 3: Implement quotes and explicit chunk mapping

**Files:**
- Create: `crates/magic-tdx-rs/src/service/quotes.rs`
- Modify: `crates/magic-tdx-rs/src/client/blocking.rs`
- Modify: `crates/magic-tdx-rs/src/client/direct.rs`
- Modify: `crates/magic-tdx-rs/src/client/asynchronous.rs`
- Modify: `crates/magic-tdx-rs/src/client/smart.rs`
- Create: `crates/magic-tdx-rs/tests/quotes_service.rs`

- [ ] **Step 1: Write cardinality and duplicate-order tests**

`quotes` must reject zero and 61 instruments and accept exactly 1/60. `quotes_chunked` must accept 121 inputs, produce three source chunks of 60/60/1, preserve repeated instrument positions, restore original input order even if responses reorder records, fail atomically when chunk 2 fails, and report every chunk endpoint/attempt/count. A missing or extra response code is `IncompletePage`/`Protocol`.

- [ ] **Step 2: Run and verify quote methods are absent**

Run: `cargo test -p magic-tdx-rs --test quotes_service`

Expected: compile failure.

- [ ] **Step 3: Implement `ChunkedBatch<T>` and mapping**

Define private-field `ChunkedBatch<T>` containing a complete `DataBatch<T>`, ordered `ChunkOutcome` metadata, and `input_to_output: Vec<usize>`. Validate response instruments against the multiset of requested `(market, code)` values; preserve duplicates by consuming response queues per key. Do not deduplicate unless a separately named future policy is registered.

- [ ] **Step 4: Implement strict and chunked quote APIs**

Expose `quotes` and `quotes_chunked` for Blocking, Direct, Async, and Smart with sync/async shape matching the client. Smart's chunked call shares one retry budget across every source chunk. A quote's `source_at` remains `None` when the packet cannot prove a trustworthy source timestamp; fetched time is still recorded separately.

- [ ] **Step 5: Run and commit quote services**

```bash
cargo test -p magic-tdx-rs --test quotes_service
cargo test -p magic-tdx-rs --test client_strategies smart_quotes_chunked
```

Expected: all pass; no 61-item strict request sends a packet.

```bash
git add crates/magic-tdx-rs/src/service/quotes.rs crates/magic-tdx-rs/src/client crates/magic-tdx-rs/src/lib.rs crates/magic-tdx-rs/tests/quotes_service.rs docs/business_rules.md
git diff --cached --check
git commit -m "feat(tdx): add explicit quote chunking"
```

### Task 4: Implement instrument, minute, and trade capabilities

**Files:**
- Create: `crates/magic-tdx-rs/src/service/instruments.rs`
- Create: `crates/magic-tdx-rs/src/service/minute.rs`
- Create: `crates/magic-tdx-rs/src/service/trades.rs`
- Modify: `crates/magic-tdx-rs/src/client/blocking.rs`
- Modify: `crates/magic-tdx-rs/src/client/direct.rs`
- Modify: `crates/magic-tdx-rs/src/client/asynchronous.rs`
- Create: `crates/magic-tdx-rs/tests/market_services.rs`

- [ ] **Step 1: Write pagination and temporal-semantic tests**

Test security count/list pagination with exact start/count and atomic all-list collection; current minute data; historical minute by date; current trades; historical trades by date/start/count; invalid date/category/count; record timestamp order; explicit no-data vs empty response; and a failed middle list/history page.

- [ ] **Step 2: Run and verify service methods are missing**

Run: `cargo test -p magic-tdx-rs --test market_services`

Expected: compile failure.

- [ ] **Step 3: Implement instrument list/count services**

Expose `security_count`, `security_list_page`, and `security_list_all` on Blocking, Direct, and Async. `list_all` obtains the declared count, fetches exact pages up to configured limits, validates unique range coverage, and errors when returned count/list coverage disagrees.

- [ ] **Step 4: Implement minute and trade services**

Expose typed current/history minute and current/history trade requests on Blocking, Direct, and Async. Validate market/code/date/start/count before sending. Use source record timestamps as `source_at` only when every record supports one unambiguous timestamp; otherwise keep batch source time absent and retain record-level raw time.

- [ ] **Step 5: Run and commit these market capabilities**

```bash
cargo test -p magic-tdx-rs --test market_services
cargo test -p magic-tdx-rs --test differential minute_
cargo test -p magic-tdx-rs --test differential trade_
```

Expected: all pass.

```bash
git add crates/magic-tdx-rs/src/service/instruments.rs crates/magic-tdx-rs/src/service/minute.rs crates/magic-tdx-rs/src/service/trades.rs crates/magic-tdx-rs/src/client crates/magic-tdx-rs/tests/market_services.rs
git diff --cached --check
git commit -m "feat(tdx): add instrument minute and trade services"
```

### Task 5: Implement finance, XDXR, and financial report capabilities

**Files:**
- Create: `crates/magic-tdx-rs/src/service/finance.rs`
- Create: `crates/magic-tdx-rs/src/service/actions.rs`
- Modify: `crates/magic-tdx-rs/src/client/blocking.rs`
- Modify: `crates/magic-tdx-rs/src/client/direct.rs`
- Modify: `crates/magic-tdx-rs/src/client/asynchronous.rs`
- Create: `crates/magic-tdx-rs/tests/finance_services.rs`

- [ ] **Step 1: Write missing-field and report-file tests**

Cover finance info, XDXR, financial file list, exact-offset report download, full report download by declared size, parsed financial records, raw/labeled indicators, missing indicator index, invalid declared file size, truncated report page, checksum/size mismatch, cache disabled, cache miss/hit/age, and stale cache rejection.

- [ ] **Step 2: Run and verify finance methods are absent**

Run: `cargo test -p magic-tdx-rs --test finance_services`

Expected: compile failure.

- [ ] **Step 3: Implement finance and corporate-action services**

Expose finance info and XDXR on Blocking, Direct, and Async. Keep absent finance fields as `Option`; never expand a short indicator vector with zeros. Return XDXR empty only when the protocol explicitly represents no corporate actions; malformed or unavailable context is an error.

- [ ] **Step 4: Implement report file list/download/parse services**

Expose report list, range download, full download, parsed records, and labeled/raw indicators at least on Blocking, using the shared blocking executor so Direct can be supported when packet semantics are identical. Full download validates declared size, bounded page coverage, filename, and optional server checksum before parse. Cache writes are atomic and cache metadata is attached to provenance; disabled remains the default.

- [ ] **Step 5: Run and commit finance capabilities**

```bash
cargo test -p magic-tdx-rs --test finance_services
cargo test -p magic-tdx-rs --test strict_failures finance_
```

Expected: all pass, including missing-as-`None` cases.

```bash
git add crates/magic-tdx-rs/src/service/finance.rs crates/magic-tdx-rs/src/service/actions.rs crates/magic-tdx-rs/src/client crates/magic-tdx-rs/tests/finance_services.rs
git diff --cached --check
git commit -m "feat(tdx): add strict finance and action services"
```

### Task 6: Implement funds, blocks, and F10/profile capabilities

**Files:**
- Create: `crates/magic-tdx-rs/src/service/funds.rs`
- Create: `crates/magic-tdx-rs/src/service/blocks.rs`
- Create: `crates/magic-tdx-rs/src/service/profile.rs`
- Modify: `crates/magic-tdx-rs/src/source/enums.rs`
- Modify: `crates/magic-tdx-rs/src/source/types.rs`
- Modify: `crates/magic-tdx-rs/src/client/blocking.rs`
- Modify: `crates/magic-tdx-rs/src/client/direct.rs`
- Create: `crates/magic-tdx-rs/tests/domain_services.rs`

- [ ] **Step 1: Write full domain-operation tests**

Funds: classify/validate fund codes, list, bars/all, quotes, minute/history minute, trades/history trades, XDXR, and finance. Blocks: remote block bars/quotes/list, industry/concept/index lists, search/list/constituents/index constituents, and local block files. Profile: categories, auto-market categories, content range, content by name, all contents, all data, and local F10 parse. Include invalid codes/names, missing category, invalid GBK, partial page, and over-limit quote input.

- [ ] **Step 2: Run and verify domain services are absent**

Run: `cargo test -p magic-tdx-rs --test domain_services`

Expected: compile failure.

- [ ] **Step 3: Implement fund source types and services**

Preserve upstream `FundInfo`, `FundBar`, `FundQuote`, `FundXdXrInfo`, `FundFinanceInfo`, and classification semantics as source types, but make invalid market/code combinations `InvalidRequest`. Reuse strict bar/quote/minute/trade/action/finance machinery so limits and failure policy remain identical.

- [ ] **Step 4: Implement block services and query operations**

Preserve typed `BlockType`, `BlockInfo`, `BlockConstituents`, `BlockRecord`, and `BlockGroup`. Remote downloads validate file names/sizes/pages; local/remote parsers share one strict implementation. Query results preserve documented sorting and duplicates and never invent an empty block on parse failure.

- [ ] **Step 5: Implement profile/F10 services**

Preserve `F10Category`, `F10Content`, and `F10Data` with bounded content reads and explicit absent-category errors. Exact-name lookup distinguishes no such category from empty category content. GBK conversion and regex parsing return typed context and do not panic on arbitrary text.

- [ ] **Step 6: Add methods only to supported executors**

Expose the full fund/block/profile suite on Blocking. Add Direct methods wherever the request is one independent TCP operation or strict page sequence; document any executor boundary in the capability matrix. Do not add `Unsupported` stubs merely to make method sets look uniform, and do not expose an `inner()` escape hatch.

- [ ] **Step 7: Run and commit domain capabilities**

```bash
cargo test -p magic-tdx-rs --test domain_services
cargo test -p magic-tdx-rs --test differential fund_
cargo test -p magic-tdx-rs --test differential block_
cargo test -p magic-tdx-rs --test differential profile_
```

Expected: every pinned valid fixture matches adopted fields; hardening cases return typed errors.

```bash
git add crates/magic-tdx-rs/src/service/funds.rs crates/magic-tdx-rs/src/service/blocks.rs crates/magic-tdx-rs/src/service/profile.rs crates/magic-tdx-rs/src/source crates/magic-tdx-rs/src/client crates/magic-tdx-rs/tests/domain_services.rs crates/magic-tdx-rs/tests/differential.rs
git diff --cached --check
git commit -m "feat(tdx): complete fund block and profile services"
```

### Task 7: Implement checked normalization and `TdxProvider`

**Files:**
- Create: `crates/magic-tdx-rs/src/adapter.rs`
- Create: `crates/magic-tdx-rs/tests/adapter.rs`
- Modify: `crates/magic-tdx-rs/src/lib.rs`

- [ ] **Step 1: Write conversion failure and source-time tests**

Test non-finite/zero/negative source price, negative volume, invalid OHLC relationship, invalid market/code/date, absent finance field, unknown ratio unit, duplicate timestamp, >20% adjacent price event, valid corporate-action discontinuity, quote without trustworthy source time, and bars with valid source timestamp. Assert conversion errors name record index/field/value and quote batch `source_at` stays absent.

- [ ] **Step 2: Run and verify the adapter is absent**

Run: `cargo test -p magic-tdx-rs --test adapter`

Expected: compile failure.

- [ ] **Step 3: Implement explicit source-to-core normalizers**

Create `Normalizer` methods for source instrument, bar, quote, trade, finance/fundamental, XDXR/corporate action, fund, block, and profile types. Each calls checked core constructors and maps failures to contextual `TdxError`; no blanket infallible `From` and no missing-to-zero conversion. Convert exchange-local source times explicitly to UTC while documenting Asia/Shanghai assumptions.

- [ ] **Step 4: Implement batch normalization and quality merge**

Convert records with indexed error context, retain original provenance fields, recompute received count, and run core validators. Define `ProviderError` in `adapter.rs` to wrap `TdxError` and `CoreError`; blocking quality issues return `ProviderError::Core(CoreError::QualityRejected { .. })` at the provider boundary unless the request explicitly asks for an observable quality report. Valid corporate-action context may explain a discontinuity but must remain visible in metadata.

- [ ] **Step 5: Implement capability traits on `TdxProvider<BlockingClient>`**

`TdxProvider` owns a configured BlockingClient and Normalizer. Implement only the actual core traits: instruments, bars, quotes, minute, trades, fundamentals, corporate actions, funds, blocks, and profile. Each calls the corresponding strict source service and returns normalized `DataBatch`. AsyncClient retains inherent async methods; do not block a Tokio runtime to force it into synchronous traits.

- [ ] **Step 6: Run and commit the core adapter**

```bash
cargo test -p magic-tdx-rs --test adapter
cargo test -p magic-market-core --test providers
```

Expected: all pass, including source-time non-substitution.

```bash
git add crates/magic-tdx-rs/src/adapter.rs crates/magic-tdx-rs/src/lib.rs crates/magic-tdx-rs/tests/adapter.rs
git diff --cached --check
git commit -m "feat(tdx): add checked core provider adapter"
```

### Task 8: Freeze the stable facade and executable capability matrix

**Files:**
- Create: `crates/magic-tdx-rs/src/prelude.rs`
- Create: `crates/magic-tdx-rs/tests/capability_matrix.rs`
- Create: `crates/magic-tdx-rs/tests/fixtures/capabilities.json`
- Modify: `crates/magic-tdx-rs/src/lib.rs`
- Modify: `crates/magic-tdx-rs/Cargo.toml`

- [ ] **Step 1: Write facade-only compile tests**

Integration tests may import only crate-root/prelude items, not internal modules. Compile representative builders and typed calls for all four clients, every source service family, local readers, `ChunkedBatch`, `TdxProvider`, errors, requests, and snapshots. Add compile-fail rustdoc for raw packet execution and pool guards, which must remain inaccessible.

- [ ] **Step 2: Create the machine-readable capability inventory**

Each JSON row has `operation`, `upstream_symbol`, `upstream_file`, `disposition` (`Adopt`, `Replaced`, or `Intentional Difference`), `facade_symbol`, `clients`, `source_type`, `normalized_trait`, `fixture`, and `test`. Populate every Rust-callable operation identified from pinned upstream; do not include an unexplained `Deferred` row.

- [ ] **Step 3: Make the inventory executable**

The test loads the JSON, rejects duplicate/missing upstream symbols, invalid dispositions, nonexistent fixture/test paths, empty client lists for network operations, and any inventory symbol absent from a checked-in public-API snapshot. Cross-check the pinned-source symbol list produced by `tools/upstream/verify.sh` so new/missed operations fail the test.

- [ ] **Step 4: Define crate-root and prelude exports**

Export stable client builders/types, typed requests/enums, source result records intended for users, readers, `TdxProvider`, errors, batch/chunk metadata, config, and snapshots. Keep codec internals, packet builders, transport, connection tasks, raw executors, pool guards, and internal protocol structs private. Enable `#![deny(missing_docs)]` and `#![deny(rustdoc::broken_intra_doc_links)]`.

- [ ] **Step 5: Run and commit facade freeze**

```bash
cargo test -p magic-tdx-rs --test capability_matrix
cargo test -p magic-tdx-rs --doc
cargo doc -p magic-tdx-rs --all-features --no-deps
```

Expected: all pass with no missing docs or broken links.

```bash
git add crates/magic-tdx-rs/src/lib.rs crates/magic-tdx-rs/src/prelude.rs crates/magic-tdx-rs/Cargo.toml crates/magic-tdx-rs/tests/capability_matrix.rs crates/magic-tdx-rs/tests/fixtures/capabilities.json
git diff --cached --check
git commit -m "feat(tdx): freeze complete Rust facade"
```

### Task 9: Add executable examples and close Phase 4

**Files:**
- Create: `crates/magic-tdx-rs/examples/blocking_bars.rs`
- Create: `crates/magic-tdx-rs/examples/async_quotes.rs`
- Create: `crates/magic-tdx-rs/examples/direct_reader.rs`
- Create: `docs/API_GUIDE.md`
- Create: `docs/DATA_MODEL.md`
- Create: `docs/ERROR_HANDLING.md`
- Modify: `.planning/2026-07-21-magic-tdx-rs/progress.md`

- [ ] **Step 1: Write compile-safe examples**

Every network example requires endpoint arguments/environment explicitly, performs only read-only market-data calls, prints provenance and typed errors, and exits non-zero on failure. The reader example operates on a caller-provided file. No example embeds a public server, fixture path, silent fallback, or `unwrap`/`expect`.

- [ ] **Step 2: Document facade, source/normalized split, and error actionability**

`API_GUIDE.md` maps capabilities to client methods and core traits. `DATA_MODEL.md` documents every unit/optional field, source-vs-normalized conversion, source/fetched time, completeness, cache, and quality outcome. `ERROR_HANDLING.md` lists every kind, context, retryability, empty/no-data distinction, atomicity, and aggregate Smart failure behavior.

- [ ] **Step 3: Run the complete Phase 4 gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo test --workspace --doc --all-features
cargo check --workspace --examples --all-features
cargo doc --workspace --all-features --no-deps
bash tools/compliance/check.sh
```

Expected: every command exits `0`; all capability inventory rows resolve to executable evidence.

- [ ] **Step 4: Record evidence, commit closeout, and stop**

```bash
git add crates/magic-tdx-rs/examples docs/API_GUIDE.md docs/DATA_MODEL.md docs/ERROR_HANDLING.md .planning/2026-07-21-magic-tdx-rs/progress.md
git diff --cached --check
git commit -m "docs: close complete service phase"
```

Record exact test commands, capability row count, source-time/strict-failure evidence, and commit SHAs. Stop for review before Phase 5.
