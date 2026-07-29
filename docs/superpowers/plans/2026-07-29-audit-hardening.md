# Audit Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `executing-plans` to implement this plan task by task with review checkpoints.

**Goal:** Reject truncated TDX packets atomically, remove Exchange request-gate
contention, machine-check BR-009 admission evidence, centralize fixed-offset
time and numeric-tolerance mechanics, and make any release-profile change
evidence-driven.

**Architecture:** Safety-sensitive byte decoding and reusable domain mechanics
move into small checked Core/TDX primitives. Providers retain source-specific
errors, policies, TLS evidence, and business units. Compliance discovers
admission constants directly from Rust source and compares them with a tracked
registry, while deterministic offline benchmarks decide whether release
settings change.

**Tech Stack:** Rust 2021 workspace, `thiserror`, `serde`, `serde_json`, `ureq`,
`magic-market-core`, `magic-market-transport`, shell/Python release tooling,
Cargo test/Clippy/Rustdoc gates.

---

## Task 1: Add checked TDX packet reads

**Files:**

- Create: `crates/magic-tdx-rs/src/protocol/cursor.rs`
- Modify: `crates/magic-tdx-rs/src/protocol/mod.rs`
- Modify: `crates/magic-tdx-rs/src/constants.rs`
- Modify: `crates/magic-tdx-rs/src/helpers.rs`
- Test: `crates/magic-tdx-rs/tests/internal/protocol_cursor.rs`
- Modify: `crates/magic-tdx-rs/tests/internal.rs`

**Step 1: Write failing cursor and helper tests**

Add tests proving:

- fixed-width reads report `RESPONSE_LENGTH_MISMATCH` with offset and field;
- bounded slices never panic;
- signed variable integers reject every unterminated encoding at the maximum
  width;
- a valid encoded zero returns zero rather than an error;
- public low-level helpers return `Result` on empty/truncated input.

Run:

```bash
cargo test -p magic-tdx-rs --test internal protocol_cursor --locked --offline
```

Expected: FAIL because the cursor does not exist and legacy helpers remain
infallible.

**Step 2: Implement the checked cursor**

Create a crate-visible `PacketCursor<'a>` with:

- `new`, `position`, `remaining`, and `is_empty`;
- checked `read_u8`, `read_u16_le`, `read_u32_le`, `read_i32_le`,
  `read_i64_le`, `read_f32_le`, and `read_slice`;
- `read_tdx_varint` with the protocol's signed value semantics and bounded
  continuation length;
- optional record context used only to enrich typed error messages.

All failures must use `ErrorCode::RESPONSE_LENGTH_MISMATCH.err(...)`; do not
substitute default values.

**Step 3: Make public low-level helpers fallible**

Change `get_byte`, fixed-width readers, and `get_price` to return the crate
`Result`. Reimplement them through the cursor or equivalent checked slices.
Update all workspace callers without `unwrap`, `expect`, or lossy fallbacks.

**Step 4: Run focused tests**

```bash
cargo test -p magic-tdx-rs --test internal protocol_cursor --locked --offline
cargo check -p magic-tdx-rs --all-targets --locked --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/magic-tdx-rs
git commit -m "fix(tdx): add checked packet cursor"
```

## Task 2: Make every TDX parser atomic

**Files:**

- Modify: `crates/magic-tdx-rs/src/protocol/parsers.rs`
- Modify: `crates/magic-tdx-rs/src/protocol/daily_bar_reader.rs`
- Modify: `crates/magic-tdx-rs/src/protocol/min_bar_reader.rs`
- Modify: other `crates/magic-tdx-rs/src/protocol/*.rs` callers found by
  `rg 'get_price|read_(u16|u32|f32|i32|i64)|get_byte'`
- Modify: `crates/magic-tdx-rs/tests/internal/protocol_parsers.rs`
- Modify: `crates/magic-tdx-rs/tests/fuzz_smoke.rs`
- Modify: `crates/magic-tdx-rs/README.md`
- Modify: `CHANGELOG.md`

**Step 1: Replace permissive truncation assertions with red tests**

For security lists, security/index bars, realtime/history minute data,
history/current transactions, and realtime quotes:

- build one valid minimum packet;
- truncate it at every byte boundary and require a typed error;
- declare two records, retain one valid record, and require whole-batch error;
- verify encoded zero remains a valid source value;
- verify each parser's documented tail rule.

Change the existing security-list test that accepts an empty partial result to
require `RESPONSE_LENGTH_MISMATCH`.

Run:

```bash
cargo test -p magic-tdx-rs --test internal protocol_parsers --locked --offline
```

Expected: FAIL on legacy `break`, zero-fill, or partial-success paths.

**Step 2: Convert parsers to exact-count cursor loops**

Each parser must:

1. check its complete header;
2. read the declared count;
3. iterate exactly `count` times with record context;
4. reject the first incomplete field;
5. validate only protocol-authorized padding or tail bytes.

Remove all truncation-driven `break` statements. Preserve explicit semantic
validation and parser-specific error codes where the packet is complete but
invalid.

**Step 3: Harden fixed-record readers**

Keep record-size preflight and replace direct indexing with fallible reads so a
future record-layout change cannot reintroduce a panic.

**Step 4: Strengthen fuzz properties**

For arbitrary input, require either:

- a typed parse error; or
- a result whose record count, consumed framing, and mandatory fields satisfy
  the parser contract.

Retain the no-panic property.

**Step 5: Document the intentional 0.x API hardening**

Record the fallible helper signatures and atomic batch behavior in the TDX
README and `CHANGELOG.md`.

**Step 6: Run TDX verification**

```bash
cargo fmt --all -- --check
cargo test -p magic-tdx-rs --all-targets --locked --offline
cargo clippy -p magic-tdx-rs --all-targets --locked --offline -- -D warnings
```

Expected: PASS.

**Step 7: Commit**

```bash
git add crates/magic-tdx-rs CHANGELOG.md
git commit -m "fix(tdx): reject truncated packets atomically"
```

## Task 3: Migrate Exchange pacing and shared policy contracts

**Files:**

- Modify: `crates/magic-exchange-rs/Cargo.toml`
- Modify: `crates/magic-exchange-rs/src/transport.rs`
- Modify: `crates/magic-exchange-rs/src/sse.rs`
- Modify: `crates/magic-exchange-rs/src/szse.rs`
- Modify: `crates/magic-exchange-rs/src/hkex.rs`
- Modify: `crates/magic-exchange-rs/src/cffex.rs`
- Modify: `crates/magic-exchange-rs/tests/transport.rs`
- Modify: `crates/magic-exchange-rs/README.md`

**Step 1: Write contention and policy red tests**

Use an injected transport that blocks the first request while recording request
start times and active-call count. Assert:

- a clone can reserve and start a later request without waiting for the first
  response to finish;
- adjacent request starts still respect the configured interval;
- maximum concurrent I/O becomes greater than one;
- a disallowed host/query key is rejected before the injected transport runs;
- oversized or invalid media responses are rejected after the transport runs;
- Rustls/native-tls selection remains visible in operator evidence.

Run:

```bash
cargo test -p magic-exchange-rs --test transport --locked --offline
```

Expected: FAIL because the local gate holds its mutex through complete I/O.

**Step 2: Add the shared transport dependency**

Add workspace `magic-market-transport` without adding any Provider dependency
to `magic-market-router`.

**Step 3: Replace local gate and compatible policy types**

Use shared `RequestGate` reservations and shared endpoint/request/response
validation. Retain the Exchange wire executor and injected transport trait where
needed to preserve explicit Rustls/native-tls selection. Ensure the call order
is:

1. validate endpoint and request;
2. wait for the reserved start;
3. execute I/O without a gate mutex;
4. validate response bounds and media type.

**Step 4: Update provider call sites and evidence**

Update SSE, SZSE, HKEX, and CFFEX callers to the new gate API. Document request
start spacing and concurrent I/O semantics without claiming async execution.

**Step 5: Run focused verification**

```bash
cargo test -p magic-market-transport --all-targets --locked --offline
cargo test -p magic-exchange-rs --all-targets --locked --offline
cargo clippy -p magic-exchange-rs --all-targets --locked --offline -- -D warnings
```

Expected: PASS.

**Step 6: Commit**

```bash
git add crates/magic-exchange-rs Cargo.lock
git commit -m "fix(exchange): release request gate before network IO"
```

## Task 4: Add machine-readable BR-009 admission enforcement

**Files:**

- Create: `docs/integrations/admissions.tsv`
- Create: `tools/compliance/check_admissions.py`
- Create: `tools/compliance/test_check_admissions.py`
- Modify: `tools/compliance/check.sh`
- Modify: `docs/integrations/README.md`

**Step 1: Write failing checker tests**

Use temporary source/docs fixtures to cover:

- source constant absent from registry;
- registry boolean drift;
- duplicate Provider/capability identity;
- admitted row with missing/out-of-scope evidence;
- admitted row with fewer than two live probes or three serial loads;
- unadmitted row without a blocker;
- one fully valid admitted and one fully valid blocked row.

Run:

```bash
python3 -m unittest tools/compliance/test_check_admissions.py
```

Expected: FAIL because the checker is absent.

**Step 2: Implement deterministic source discovery**

Scan workspace Rust files for public constants ending in `_ADMITTED` whose
literal is `true` or `false`. Parse a TSV registry with a fixed header and
compare exact crate/constant/boolean identities. Reject duplicates, unknown
rows, missing rows, invalid counts/dates/status, and invalid evidence paths.

The checker must read only tracked source/document files and never invoke a
network probe.

**Step 3: Populate every current admission constant**

Register CFETS, NBS, Xinhua, PBC, World Bank, IMF, FRED, Yicai,
WallstreetCN, STCN, SEC, and any additional constant discovered by the checker.
Use existing integration evidence dates/counts for admitted capabilities and
the current explicit source blocker for every false row. Do not change any Rust
admission boolean.

**Step 4: Wire the checker into compliance**

Run unit tests first, then call `check_admissions.py` from
`tools/compliance/check.sh`. Add the registry/checker to required release
artifacts if the existing script maintains such a list.

**Step 5: Verify**

```bash
python3 -m unittest tools/compliance/test_check_admissions.py
bash tools/compliance/check.sh
```

Expected: PASS.

**Step 6: Commit**

```bash
git add docs/integrations tools/compliance
git commit -m "feat(compliance): enforce BR-009 admission registry"
```

## Task 5: Centralize fixed-offset time and numeric tolerances

**Files:**

- Create: `crates/magic-market-core/src/time.rs`
- Modify: `crates/magic-market-core/src/lib.rs`
- Modify: `crates/magic-market-core/src/value.rs`
- Modify: `crates/magic-market-core/src/capital.rs`
- Modify: `crates/magic-market-core/src/provider.rs`
- Modify: `crates/magic-market-core/src/signals.rs`
- Create: `crates/magic-market-core/tests/time.rs`
- Modify: `crates/magic-market-core/tests/values.rs`
- Modify: `crates/magic-eastmoney-rs/src/post_close.rs`
- Modify: `crates/magic-eastmoney-rs/src/market_rankings.rs`
- Modify: `crates/magic-ths-rs/src/lib.rs`
- Modify: `crates/magic-cninfo-rs/src/lib.rs`
- Modify: `crates/magic-cls-rs/src/lib.rs`
- Modify: `crates/magic-thepaper-rs/src/lib.rs`
- Modify: equivalent Provider files discovered by
  `rg 'civil_from_days|unix_seconds_to_.*iso' crates`
- Modify: `crates/magic-tencent-rs/src/lib.rs`
- Modify: `crates/magic-sina-rs/src/lib.rs`
- Modify: `crates/magic-exchange-rs/src/szse.rs`

**Step 1: Add red Core time tests**

Cover Unix epoch, China midnight/day rollover, leap day, negative epoch,
minimum/maximum supported arithmetic, invalid offsets, strict clock grammar,
clock ordering, and compatibility with `EvidenceTimestamp::parse`.

Run:

```bash
cargo test -p magic-market-core --test time --locked --offline
```

Expected: FAIL because the shared utilities do not exist.

**Step 2: Implement checked time values**

Add:

- checked fixed-offset Unix-seconds conversion to canonical RFC3339;
- `unix_seconds_to_china_rfc3339`;
- a strict ordered clock value accepting exactly `HH:MM:SS`.

Use checked arithmetic and Gregorian bounds. Export only the types/functions
needed by Providers.

**Step 3: Migrate duplicate provider converters and BR-019**

Remove hand-written `civil_from_days` copies after migrating every matching
Provider. Map Core errors into each Provider's existing typed error. Replace
BR-019 lexical clock comparison with the strict clock type.

Run:

```bash
rg 'fn civil_from_days|fn unix_seconds_to_.*iso' crates
cargo test -p magic-market-core -p magic-eastmoney-rs -p magic-ths-rs \
  -p magic-cninfo-rs -p magic-cls-rs -p magic-thepaper-rs --locked --offline
```

Expected: no duplicate functions and all tests PASS.

**Step 4: Add red tolerance tests**

Cover invalid negative/NaN/infinite components, absolute-only cent behavior,
relative large-number behavior, combined trade-amount behavior, symmetry,
zero, infinities/NaN inputs, and exact boundary inclusion.

**Step 5: Implement `NumericTolerance`**

Add a checked value with named constructors or constants for mechanics only.
`matches` rejects non-finite operands and applies:

```text
abs(left - right) <= absolute + relative * max(abs(left), abs(right))
```

Keep exact `PartialEq` unchanged on existing domain values.

**Step 6: Migrate named business policies**

Replace ad-hoc comparison mechanics while retaining call-site names and units:

- Core money reconciliation: one cent absolute;
- Core order-book aggregation: existing relative summation scale;
- Tencent percentage points and trade-amount contract;
- Sina top-of-book precision;
- SZSE source-decimal precision.

Run:

```bash
cargo test -p magic-market-core -p magic-tencent-rs -p magic-sina-rs \
  -p magic-exchange-rs --all-targets --locked --offline
cargo clippy -p magic-market-core -p magic-tencent-rs -p magic-sina-rs \
  -p magic-exchange-rs --all-targets --locked --offline -- -D warnings
```

Expected: PASS with source-specific acceptance boundaries unchanged.

**Step 7: Commit**

```bash
git add crates
git commit -m "refactor(core): centralize time and numeric policies"
```

## Task 6: Measure release-profile candidates

**Files:**

- Modify: `crates/magic-tdx-rs/examples/parse_bench.rs`
- Create: `tools/bench/release_profile.sh`
- Create: `tools/bench/compare_release_profiles.py`
- Create: `tools/bench/test_compare_release_profiles.py`
- Modify: `docs/PERFORMANCE_RESULTS.md`
- Conditionally modify: `Cargo.toml`

**Step 1: Write red comparison-policy tests**

Test identical/mismatched checksums, combined median below/above five percent,
an individual workload regression above five percent, and binary growth above
twenty percent.

Run:

```bash
python3 -m unittest tools/bench/test_compare_release_profiles.py
```

Expected: FAIL because the comparison tool is absent.

**Step 2: Implement deterministic offline benchmark output**

Extend the benchmark to emit machine-readable records for:

- TDX variable decoding/bar parsing;
- JSON decode/normalization;
- bounded compression/decompression when existing dependencies and fixtures
  support it.

Each record includes workload, iterations, elapsed nanoseconds, throughput, and
checksum. Inputs are fixed and no network or wall-clock source data is used.

**Step 3: Implement the profile runner and policy checker**

Build separate default/candidate target directories, perform one warm-up, run
five alternating measured rounds, record executable size, and evaluate the
approved thresholds. The tool must exit nonzero when the candidate does not
qualify and print a structured reason.

**Step 4: Run the exact-revision measurement**

```bash
bash tools/bench/release_profile.sh
```

Expected: complete offline evidence for both profiles with identical
checksums.

**Step 5: Apply the measured decision**

Only if every threshold passes, add:

```toml
[profile.release]
lto = "thin"
codegen-units = 1
```

Otherwise leave the workspace profile unchanged. Record command, environment,
five-run medians, workload regressions, checksums, sizes, and decision in
`docs/PERFORMANCE_RESULTS.md`.

**Step 6: Verify benchmark tooling**

```bash
python3 -m unittest tools/bench/test_compare_release_profiles.py
cargo check -p magic-tdx-rs --example parse_bench --release --locked --offline
```

Expected: PASS.

**Step 7: Commit**

```bash
git add Cargo.toml crates/magic-tdx-rs/examples tools/bench \
  docs/PERFORMANCE_RESULTS.md
git commit -m "perf: add evidence-driven release profile benchmark"
```

## Task 7: Run Gates A through D and review

**Files:**

- Modify only files required to correct a verified gate or review failure.
- Update: `.planning/2026-07-29-audit-hardening/task_plan.md`
- Update: `.planning/2026-07-29-audit-hardening/findings.md`
- Update: `.planning/2026-07-29-audit-hardening/progress.md`

**Step 1: Run formatting and workspace compilation**

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked --offline
```

Expected: PASS.

**Step 2: Run tests and strict Clippy**

```bash
cargo test --workspace --all-targets --all-features --locked --offline
cargo clippy --workspace --all-targets --all-features --locked --offline \
  -- -D warnings
```

Expected: PASS; only documented live-network tests remain ignored.

**Step 3: Run documentation and compliance**

```bash
cargo doc --workspace --all-features --no-deps --locked --offline
cargo test --workspace --doc --all-features --locked --offline
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
```

Expected: PASS.

**Step 4: Run coverage, package, and release preflight**

Run the repository's tracked critical-path coverage command, then:

```bash
bash tools/release/package.sh
bash tools/release/preflight.sh
```

Expected: coverage thresholds and both release scripts PASS.

**Step 5: Perform final review**

Inspect the complete diff for:

- silent fallbacks, partial success, panic/unwrap in new production code;
- changed business units or source identities;
- Router dependency-direction regressions;
- untracked benchmark/admission claims;
- unrelated user changes.

Resolve every Critical or Important finding and rerun the affected focused test
plus the complete gate set.

**Step 6: Commit release evidence**

```bash
git add .planning docs tools crates Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore: record audit hardening verification"
```

**Step 7: Prepare integration**

Report the branch commit range, exact gate evidence, benchmark decision, and
remaining external live-admission blockers. Integrate only through the
repository's approved branch workflow.
