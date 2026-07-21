# Phase 2: Protocol, Adjustment, and Local Readers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver bounds-checked TDX source decoding, packet construction, strict adjustment, and every pinned-upstream local reader with auditable provenance and deterministic compatibility evidence.

**Architecture:** All byte access flows through one contextual cursor and all decompression is size-bounded. Target modules are audited extractions from the fixed upstream commit, split by record family; source records preserve upstream fields and `f64` values, while malformed, truncated, incomplete, or ambiguous inputs return typed errors instead of zero/default/panic behavior.

**Tech Stack:** Rust 1.83, Serde, thiserror, flate2, encoding_rs, regex, sha2, proptest, cargo-fuzz, pinned `tdxrs` commit `18b05ffc9d8a257b5ba5add8a2d1ab038261747d`.

---

## Exit gate

Phase 2 is complete only when the source inventory has no unreviewed protocol/reader file, fixtures carry hashes and origin, strict-failure tests cover every identified silent/default gap, fixed-input differential tests pass, and arbitrary/truncated inputs are demonstrated not to panic or allocate beyond configured bounds. Network clients remain out of scope.

### Task 1: Make the pinned upstream baseline reproducible and auditable

**Files:**
- Create: `tools/upstream/fetch.sh`
- Create: `tools/upstream/verify.sh`
- Create: `provenance/pure-rust.patch`
- Modify: `provenance/upstream-files.toml`
- Create: `docs/UPSTREAM.md`
- Create: `crates/magic-tdx-rs/tests/fixtures/manifest.json`
- Create: `crates/magic-tdx-rs/tests/fixtures/protocol/.gitkeep`
- Create: `crates/magic-tdx-rs/tests/fixtures/readers/.gitkeep`
- Create: `crates/magic-tdx-rs/tests/golden.rs`

- [ ] **Step 1: Write the upstream verification script first**

`tools/upstream/verify.sh` takes the checkout directory as its only positional argument and must fail unless all of these are exact: Git remote URL, HEAD commit, clean tracked worktree before patching, package version `0.6.7`, MIT license digest, and the SHA-256 of `provenance/pure-rust.patch`. It then applies the patch with `git apply --check`, rejects hunks outside `Cargo.toml`, `src/lib.rs`, and `src/python/`, and runs `cargo test --lib --no-default-features` in the patched checkout.

- [ ] **Step 2: Run verification against an intentionally wrong directory**

Run: `bash tools/upstream/verify.sh .`

Expected: non-zero exit with a message naming the expected pinned commit.

- [ ] **Step 3: Implement deterministic fetch and the minimal PyO3-decoupling patch**

`tools/upstream/fetch.sh` takes one newly created empty destination directory, clones `https://github.com/jiangtaovan/tdxrs`, detaches exactly at `18b05ffc9d8a257b5ba5add8a2d1ab038261747d`, and calls `verify.sh`. The committed patch may only remove the `cdylib`, PyO3 dependency/registration, and Python module compilation. It must not change `protocol`, `net`, `reader`, `fund`, `block`, `profile`, constants, helpers, numeric logic, tests, or benches.

- [ ] **Step 4: Populate the provenance inventory before extracting code**

For every adopted source file, add a row with this schema:

```toml
[[files]]
upstream = "src/protocol/parsers.rs"
target = "crates/magic-tdx-rs/src/protocol/parsers"
disposition = "split-and-harden"
upstream_sha256 = "43510d3304115f77adaae8f532ff7669ce83368552d7047b6f5af214d2f25758"
notes = "All byte reads replaced by ByteCursor; strict length/count semantics."
```

The shown digest is the pinned `src/protocol/parsers.rs` value and must be reverified by the script before commit. Calculate and store corresponding 64-character digests for every other inventoried file. Inventory every adopted file under upstream `protocol`, `reader`, `fund`, `block`, and `profile`; use `rejected-python` rows for excluded Python modules and `replaced` rows for unsafe/defaulting helpers.

- [ ] **Step 5: Define the fixture manifest schema and validation**

Use JSON entries with `path`, `sha256`, `upstream_commit`, `origin`, `operation`, `request_hex` (nullable for local files), `expected_record_count`, and `contains_live_data`. Add a Rust test that loads every entry, rejects missing files/digest mismatch/unpinned commits, and asserts live-derived fixtures are documented and contain no account/order data.

- [ ] **Step 6: Verify and commit provenance infrastructure**

Run:

```bash
tmp_checkout=$(mktemp -d /tmp/magic-tdx-upstream.XXXXXX)
bash tools/upstream/fetch.sh "$tmp_checkout"
cargo test -p magic-tdx-rs --test golden fixture_manifest_is_complete
bash tools/compliance/check.sh
```

Expected: all commands exit `0`; `git diff -- provenance/pure-rust.patch` shows no protocol/numeric change.

```bash
git add tools/upstream provenance docs/UPSTREAM.md crates/magic-tdx-rs/tests/fixtures crates/magic-tdx-rs/tests/golden.rs
git diff --cached --check
git commit -m "build: pin auditable tdxrs source baseline"
```

### Task 2: Implement typed contextual errors and bounds-checked codecs

**Files:**
- Create: `crates/magic-tdx-rs/src/error.rs`
- Create: `crates/magic-tdx-rs/src/codec/mod.rs`
- Create: `crates/magic-tdx-rs/src/codec/cursor.rs`
- Create: `crates/magic-tdx-rs/src/codec/varint.rs`
- Create: `crates/magic-tdx-rs/src/codec/decompress.rs`
- Create: `crates/magic-tdx-rs/tests/strict_failures.rs`
- Modify: `crates/magic-tdx-rs/src/lib.rs`
- Modify: `crates/magic-tdx-rs/Cargo.toml`

- [ ] **Step 1: Write failing cursor and decompression tests**

```rust
use magic_tdx_rs::{ByteCursor, ErrorKind, Limits};

#[test]
fn short_read_reports_field_and_offset() {
    let mut cursor = ByteCursor::new("quote", &[0x01]);
    let error = cursor.read_u32_le("price").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Decode);
    assert_eq!(error.context().field(), Some("price"));
    assert_eq!(error.context().offset(), Some(0));
}

#[test]
fn decompression_respects_output_limit() {
    let limits = Limits::builder().max_decompressed_bytes(16).build().unwrap();
    let compressed = include_bytes!("fixtures/protocol/zlib-more-than-16.bin");
    assert!(matches!(magic_tdx_rs::decompress_zlib(compressed, &limits), Err(error) if error.kind() == ErrorKind::Decompression));
}
```

- [ ] **Step 2: Run the focused test and verify it fails to compile**

Run: `cargo test -p magic-tdx-rs --test strict_failures short_read`

Expected: unresolved public codec/error types.

- [ ] **Step 3: Implement `TdxError`, `ErrorContext`, and retryability**

Define non-exhaustive `ErrorKind` with the design's exact families: `Configuration`, `InvalidRequest`, `Transport`, `Protocol`, `Decode`, `Decompression`, `RateLimited`, `PoolExhausted`, `NoData`, `EmptyResponse`, `IncompletePage`, `Adjustment`, `RetryExhausted`, and `Unsupported`. `TdxError` stores kind, retryable flag, message, operation, instrument, endpoint, attempt, page, field, offset, request id, and optional source error. Expose read-only accessors and constructors scoped by family; no caller parses strings to decide retry.

- [ ] **Step 4: Implement `ByteCursor` and varint decoding**

`ByteCursor<'a>` stores operation, input, and position. Implement checked `read_u8`, `read_i16_le`, `read_u16_le`, `read_i32_le`, `read_u32_le`, `read_i64_le`, `read_f32_le`, `read_exact`, `skip`, `remaining`, `position`, and `finish(allow_trailing)`. Every read uses checked addition and reports the field/start offset. Implement TDX price varint with a maximum encoded width and overflow errors; never return zero because input ended.

- [ ] **Step 5: Implement configured limits and bounded zlib decoding**

`Limits` has validated private fields for maximum response bytes, decompressed bytes, local-file bytes, records, pages, and allocation bytes. Stream zlib output into a buffer that stops before exceeding the configured maximum; validate declared compressed/uncompressed sizes before allocation.

- [ ] **Step 6: Add property tests for arbitrary and truncated input**

For every primitive read and varint decoder, generate arbitrary byte vectors of length 0–64. Wrap calls in `catch_unwind`, assert no panic, and assert cursor position never exceeds input length. Generate compressed streams whose expansion crosses limits and assert `Decompression` without a large retained buffer.

- [ ] **Step 7: Run and commit the codec gate**

```bash
cargo test -p magic-tdx-rs --test strict_failures
cargo test -p magic-tdx-rs codec
cargo clippy -p magic-tdx-rs --all-targets -- -D warnings
```

Expected: all pass.

```bash
git add crates/magic-tdx-rs/Cargo.toml crates/magic-tdx-rs/src/error.rs crates/magic-tdx-rs/src/codec crates/magic-tdx-rs/src/lib.rs crates/magic-tdx-rs/tests/strict_failures.rs crates/magic-tdx-rs/tests/fixtures/protocol
git diff --cached --check
git commit -m "feat(tdx): add bounded codecs and contextual errors"
```

### Task 3: Implement source types, response headers, and packet builders

**Files:**
- Create: `crates/magic-tdx-rs/src/source/mod.rs`
- Create: `crates/magic-tdx-rs/src/source/enums.rs`
- Create: `crates/magic-tdx-rs/src/source/types.rs`
- Create: `crates/magic-tdx-rs/src/protocol/mod.rs`
- Create: `crates/magic-tdx-rs/src/protocol/header.rs`
- Create: `crates/magic-tdx-rs/src/protocol/packet.rs`
- Create: `crates/magic-tdx-rs/tests/golden.rs`
- Modify: `crates/magic-tdx-rs/src/lib.rs`

- [ ] **Step 1: Write failing response-header and request-validation tests**

```rust
use magic_tdx_rs::{Adjustment, BarCategory, Market, PacketBuilder, ResponseHeader};

#[test]
fn response_header_rejects_declared_body_larger_than_available() {
    let bytes = include_bytes!("fixtures/protocol/truncated-response.bin");
    assert!(ResponseHeader::decode(bytes).is_err());
}

#[test]
fn packet_builder_rejects_magic_integers_and_bad_codes() {
    assert!(PacketBuilder::bars(Market::Shanghai, "600000", BarCategory::Day, 0, 10, Adjustment::None).is_ok());
    assert!(PacketBuilder::bars(Market::Shanghai, "TOO-LONG", BarCategory::Day, 0, 10, Adjustment::None).is_err());
}
```

- [ ] **Step 2: Run tests and verify unresolved types**

Run: `cargo test -p magic-tdx-rs --test golden response_header`

Expected: compile failure.

- [ ] **Step 3: Define source enums and record types**

Create non-exhaustive checked enums `Market`, `BarCategory`, `SecurityKind`, and `TradeDirection`, and reuse/re-export the provider-neutral core `Adjustment` enum rather than defining a conflicting TDX copy. Preserve source-facing records equivalent to upstream `SecurityBar`, `IndexBar`, `SecurityQuote`, `SecurityInfo`, `MinuteTimePrice`, `TickData`, `FinanceInfo`, `XdXrInfo`, and `BlockInfoMeta`. Retain verified upstream numeric fields and unknown raw fields; document units and do not add normalized core value types here.

- [ ] **Step 4: Decode and validate response headers**

Extract upstream header layout into `ResponseHeader::decode`. Validate magic/type bytes, compressed and decompressed lengths, body availability, configured limits, and exact body slicing. Expose only safe body bytes, never internal unchecked offsets.

- [ ] **Step 5: Implement typed packet builders**

Implement builders for every pinned request opcode used by bars/index bars, quotes, list/count, minute/history minute, trades/history trades, finance, XDXR, block info, financial report, fund, and F10 services. Each builder validates six-byte codes, start/count ranges, file names, block names, and the 60-quote limit. Builders return owned immutable request bytes plus operation metadata for later error context.

- [ ] **Step 6: Compare request bytes with pinned upstream golden bytes**

For each opcode family, store one request fixture and assert byte-for-byte equality. The fixture manifest records the upstream function and parameters used to generate it. Include boundary tests at count 1, maximum accepted count, zero, and maximum+1.

- [ ] **Step 7: Run and commit protocol framing**

```bash
cargo test -p magic-tdx-rs --test golden request_
cargo test -p magic-tdx-rs protocol
```

Expected: all request golden tests pass.

```bash
git add crates/magic-tdx-rs/src/source crates/magic-tdx-rs/src/protocol crates/magic-tdx-rs/src/lib.rs crates/magic-tdx-rs/tests/golden.rs crates/magic-tdx-rs/tests/fixtures/protocol provenance/upstream-files.toml
git diff --cached --check
git commit -m "feat(tdx): add typed source protocol framing"
```

### Task 4: Extract and harden every protocol parser

**Files:**
- Create: `crates/magic-tdx-rs/src/protocol/parsers/mod.rs`
- Create: `crates/magic-tdx-rs/src/protocol/parsers/bars.rs`
- Create: `crates/magic-tdx-rs/src/protocol/parsers/quotes.rs`
- Create: `crates/magic-tdx-rs/src/protocol/parsers/instruments.rs`
- Create: `crates/magic-tdx-rs/src/protocol/parsers/minute.rs`
- Create: `crates/magic-tdx-rs/src/protocol/parsers/trades.rs`
- Create: `crates/magic-tdx-rs/src/protocol/parsers/finance.rs`
- Create: `crates/magic-tdx-rs/src/protocol/parsers/xdxr.rs`
- Create: `crates/magic-tdx-rs/src/protocol/parsers/block.rs`
- Create: `crates/magic-tdx-rs/tests/differential.rs`
- Modify: `crates/magic-tdx-rs/tests/golden.rs`
- Modify: `crates/magic-tdx-rs/tests/strict_failures.rs`

- [ ] **Step 1: Add one failing strict test per parser family**

The table-driven test cases must include: truncated field, declared count mismatch, invalid date/time, invalid UTF-8/GBK conversion, invalid finance field count, and unexpected trailing bytes. Each asserts `ErrorKind`, field, and offset, rather than only `is_err()`.

- [ ] **Step 2: Run the parser strict tests and verify they fail**

Run: `cargo test -p magic-tdx-rs --test strict_failures parser_`

Expected: failures because parser functions are not implemented.

- [ ] **Step 3: Port bars, instruments, minute, and trade parsing through `ByteCursor`**

Preserve upstream field formulas and scaling, but require exact declared records and valid encoded dates/times. Keep the transaction `reserved` value raw and documented as unknown. Do not use the upstream short-read helpers or unchecked slices.

- [ ] **Step 4: Port quote, finance, XDXR, and block parsing through `ByteCursor`**

For quotes, preserve every price/volume/order-book field and reject incomplete records. For finance, represent absent indexed fields as `None` in the source record instead of `0.0`; an invalid finance file size is a decode error. XDXR and block parsers require full declared consumption and bounded decompression.

- [ ] **Step 5: Add full-field golden and differential assertions**

Golden tests deserialize expected JSON and compare every source field, not just record count. Differential tests run the patched pinned-upstream parser against the same fixture and compare all fields that are intentionally adopted. For hardened differences, assert the upstream result and target error in a named test and link the case from `docs/COMPATIBILITY.md` in Phase 5.

- [ ] **Step 6: Verify parser properties**

Generate truncations at every byte boundary for each golden fixture. Every prefix shorter than the complete payload must either return a typed error or a protocol-documented complete shorter message; no prefix may panic. Reject declared record counts above `Limits::max_records` before allocating.

- [ ] **Step 7: Run and commit parser extraction**

```bash
cargo test -p magic-tdx-rs --test golden parser_
cargo test -p magic-tdx-rs --test differential parser_
cargo test -p magic-tdx-rs --test strict_failures parser_
```

Expected: adopted fixtures match all fields; intentional strict differences pass their error assertions.

```bash
git add crates/magic-tdx-rs/src/protocol/parsers crates/magic-tdx-rs/tests crates/magic-tdx-rs/tests/fixtures/protocol provenance/upstream-files.toml
git diff --cached --check
git commit -m "feat(tdx): harden complete protocol parsing"
```

### Task 5: Implement complete and atomic adjustment

**Files:**
- Create: `crates/magic-tdx-rs/src/adjustment/mod.rs`
- Create: `crates/magic-tdx-rs/src/adjustment/factor.rs`
- Create: `crates/magic-tdx-rs/src/adjustment/service.rs`
- Create: `crates/magic-tdx-rs/tests/adjustment.rs`
- Modify: `crates/magic-tdx-rs/src/lib.rs`

- [ ] **Step 1: Write failure-first adjustment tests**

```rust
#[test]
fn adjusted_request_fails_when_any_context_page_fails() {
    let source = ScriptedAdjustmentSource::new([Ok(page(0)), Err(protocol_error("page 1"))]);
    let error = AdjustmentService::new(source).adjust(request_for_front_adjustment()).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Adjustment);
    assert_eq!(error.context().failed_page(), Some(1));
}

#[test]
fn no_adjustment_never_fetches_xdxr() {
    let source = CountingAdjustmentSource::default();
    AdjustmentService::new(&source).adjust(request_without_adjustment()).unwrap();
    assert_eq!(source.xdxr_calls(), 0);
}
```

- [ ] **Step 2: Run and verify the focused failures**

Run: `cargo test -p magic-tdx-rs --test adjustment`

Expected: compile failure before the service exists.

- [ ] **Step 3: Port factor calculations with full-field differential cases**

Extract upstream `FqFactorItem`, `FqFactorResult`, `calc_fq_factors`, and security-bar adjustment formulas into private target types/functions. Add fixtures for cash dividend, bonus shares, rights issue, same-day multiple actions, no action, front adjustment, and back adjustment. Compare every adjusted OHLC field and factor with pinned upstream within a documented floating tolerance.

- [ ] **Step 4: Implement an atomic context contract**

Define an internal `AdjustmentSource` trait that returns page outcomes with page number and expected range. `AdjustmentService` fetches all required XDXR and context-bar pages under one limits/deadline contract; any error, gap, duplicate, invalid action, or early empty page returns `Adjustment`/`IncompletePage` and discards the provisional result. `Adjustment::None` bypasses context entirely.

- [ ] **Step 5: Add the upstream silent-downgrade regression tests**

Exercise XDXR transport error, XDXR parse error, missing context page, invalid corporate action, and exhausted empty context. Assert no test returns the original unadjusted bars as a success for an adjusted request.

- [ ] **Step 6: Run and commit adjustment**

```bash
cargo test -p magic-tdx-rs --test adjustment
cargo test -p magic-tdx-rs --test differential adjustment_
```

Expected: all pass.

```bash
git add crates/magic-tdx-rs/src/adjustment crates/magic-tdx-rs/src/lib.rs crates/magic-tdx-rs/tests/adjustment.rs crates/magic-tdx-rs/tests/differential.rs crates/magic-tdx-rs/tests/fixtures/protocol provenance/upstream-files.toml
git diff --cached --check
git commit -m "feat(tdx): make adjustment context atomic"
```

### Task 6: Extract and harden all local file readers

**Files:**
- Create: `crates/magic-tdx-rs/src/reader/mod.rs`
- Create: `crates/magic-tdx-rs/src/reader/daily.rs`
- Create: `crates/magic-tdx-rs/src/reader/min.rs`
- Create: `crates/magic-tdx-rs/src/reader/financial.rs`
- Create: `crates/magic-tdx-rs/src/reader/block.rs`
- Create: `crates/magic-tdx-rs/src/reader/profile.rs`
- Create: `crates/magic-tdx-rs/tests/readers.rs`
- Modify: `crates/magic-tdx-rs/tests/differential.rs`

- [ ] **Step 1: Write exact-size and corrupt-file tests first**

For daily, min/lc, financial, block, TNF/security-list, and F10/profile fixtures, test an empty file, one complete record, trailing partial record, invalid declared size, invalid GBK, maximum allowed file, and one byte above the configured limit. Partial fixed-width records must return `Decode`, never be ignored.

- [ ] **Step 2: Run and verify reader tests fail**

Run: `cargo test -p magic-tdx-rs --test readers`

Expected: compile failure before readers exist.

- [ ] **Step 3: Port daily and minute readers**

Use exact record-width checks and `ByteCursor`. Preserve upstream scaling/date formulas, validate dates and finite values, and return source records. File APIs accept `AsRef<Path>`, attach the path and byte offset to errors, and reject files over `Limits::max_local_file_bytes` before reading the whole file.

- [ ] **Step 4: Port financial, block, security-list, and profile readers**

Replace missing finance indexes with `Option`, validate file/header/record sizes, use strict GBK decoding policy documented per field, and keep unknown bytes/raw fields where semantics are unproven. Regex-based F10 parsing must be bounded by input size and return explicit missing-category/content outcomes.

- [ ] **Step 5: Add field-complete differential and corruption tests**

For every valid fixture compare all target fields against patched pinned upstream. For every corrupted fixture assert target typed failure; when upstream truncates/defaults, record it as an intentional difference in the test name and fixture manifest.

- [ ] **Step 6: Add reader Criterion benches without acceptance claims**

Create `benches/reader.rs` covering daily, min, finance, and block fixtures using `black_box`; record throughput in bytes and records. This phase only establishes the harness—Phase 5 performs controlled A/B and applies the 5% gate.

- [ ] **Step 7: Run and commit all readers**

```bash
cargo test -p magic-tdx-rs --test readers
cargo test -p magic-tdx-rs --test differential reader_
cargo bench -p magic-tdx-rs --bench reader --no-run
```

Expected: tests pass and the benchmark compiles.

```bash
git add crates/magic-tdx-rs/src/reader crates/magic-tdx-rs/tests/readers.rs crates/magic-tdx-rs/tests/differential.rs crates/magic-tdx-rs/tests/fixtures/readers crates/magic-tdx-rs/benches/reader.rs crates/magic-tdx-rs/Cargo.toml provenance/upstream-files.toml
git diff --cached --check
git commit -m "feat(tdx): add strict local data readers"
```

### Task 7: Add fuzz targets and close the protocol phase

**Files:**
- Create: `crates/magic-tdx-rs/fuzz/Cargo.toml`
- Create: `crates/magic-tdx-rs/fuzz/fuzz_targets/decode_response.rs`
- Create: `crates/magic-tdx-rs/fuzz/fuzz_targets/parse_bars.rs`
- Create: `crates/magic-tdx-rs/fuzz/fuzz_targets/parse_quotes.rs`
- Create: `crates/magic-tdx-rs/fuzz/fuzz_targets/readers.rs`
- Modify: `docs/PROTOCOL.md`
- Modify: `.planning/2026-07-21-magic-tdx-rs/progress.md`

- [ ] **Step 1: Implement bounded fuzz entry points**

Each target rejects corpus items above 1 MiB, constructs strict small `Limits`, invokes exactly one decoder/parser, and ignores typed `Err` while allowing panics, timeouts, or sanitizer failures to fail the run. Seed corpora from every golden fixture and all one-byte truncations.

- [ ] **Step 2: Run deterministic fuzz smoke tests**

Run each target for 10,000 iterations with a 2-second per-input timeout:

```bash
cargo fuzz run decode_response -- -runs=10000 -timeout=2
cargo fuzz run parse_bars -- -runs=10000 -timeout=2
cargo fuzz run parse_quotes -- -runs=10000 -timeout=2
cargo fuzz run readers -- -runs=10000 -timeout=2
```

Expected: no crash, timeout, out-of-memory result, or generated panic artifact.

- [ ] **Step 3: Document protocol evidence and unknowns**

`docs/PROTOCOL.md` must map every supported opcode/record to target parser, upstream file/commit, fixture, strict differences, unit/scaling, trailing-byte policy, and unknown raw fields. Explicitly mark transaction `reserved` and every unproven field as unknown.

- [ ] **Step 4: Run the complete Phase 2 gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo test --workspace --doc --all-features
cargo doc --workspace --all-features --no-deps
bash tools/compliance/check.sh
```

Expected: every command exits `0`; default tests use only committed local fixtures.

- [ ] **Step 5: Record evidence, commit closeout, and stop**

```bash
git add crates/magic-tdx-rs/fuzz docs/PROTOCOL.md .planning/2026-07-21-magic-tdx-rs/progress.md
git diff --cached --check
git commit -m "test(tdx): close protocol compatibility phase"
```

Record exact fixture digests, upstream checkout digest, fuzz commands/results, validation commands, and commit SHAs in `progress.md`. Stop for review before Phase 3.
