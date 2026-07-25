# Audit Correctness and Release Gates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Correct the verified Eastmoney and TDX defects and make Clippy,
coverage, and release-build gates truthful without weakening data contracts or
thresholds.

**Architecture:** Keep exchange and quote validation at provider normalization
boundaries, add one private TDX synchronization utility for typed and
compatibility-safe lock handling, and refactor server probing around a private
fallible session seam. Restore the documented coverage checker first with
synthetic tests, then close real coverage gaps with branch-matrix and
disconnected/loopback protocol tests until the exact CI report reaches 80%
overall and 95% for the combined critical set.

**Tech Stack:** Rust 2021 workspace, standard-library TCP and synchronization,
Python 3 `unittest`, `cargo clippy`, `cargo llvm-cov`, Bash release gates.

---

## File Map

**Create**

- `crates/magic-tdx-rs/src/sync.rs` — typed and warning-backed mutex helpers.
- `docs/superpowers/plans/2026-07-25-audit-correctness-gates.md` — this plan.

**Modify for correctness**

- `crates/magic-eastmoney-rs/src/lib.rs` — exact 920-prefix validation and
  regression tests.
- `crates/magic-tdx-rs/src/adapter.rs` — contextual required quote-price
  validation, shared order-book normalizer, and branch tests.
- `crates/magic-tdx-rs/src/lib.rs` — register the sync module and remove the
  blanket Clippy allowance.
- `crates/magic-tdx-rs/src/block/client.rs` — typed/recovering lock use.
- `crates/magic-tdx-rs/src/net/client.rs` — fallible probe session and lock use.
- `crates/magic-tdx-rs/src/net/pool.rs` — typed/recovering lock use.
- `crates/magic-tdx-rs/src/net/smart_client.rs` — typed/recovering lock use.
- `crates/magic-tdx-rs/src/net/utils.rs` — recovering limiter locks.
- `crates/magic-tdx-rs/src/service/mod.rs` — reuse the shared order-book
  normalizer.

**Modify for Clippy**

- `crates/magic-tdx-rs/src/block/query.rs`
- `crates/magic-tdx-rs/src/block/types.rs`
- `crates/magic-tdx-rs/src/constants.rs`
- `crates/magic-tdx-rs/src/helpers.rs`
- `crates/magic-tdx-rs/src/net/async_client.rs`
- `crates/magic-tdx-rs/src/net/direct_client.rs`
- `crates/magic-tdx-rs/src/net/finance_client.rs`
- `crates/magic-tdx-rs/src/profile/constants.rs`
- `crates/magic-tdx-rs/src/protocol/adjuster.rs`
- `crates/magic-tdx-rs/src/protocol/constants.rs`
- `crates/magic-tdx-rs/src/protocol/fq_service.rs`
- `crates/magic-tdx-rs/src/protocol/parsers.rs`
- `crates/magic-tdx-rs/src/reader/block.rs`
- `crates/magic-tdx-rs/src/reader/daily_bar.rs`
- `crates/magic-tdx-rs/src/reader/min_bar.rs`

**Modify for coverage and release**

- `crates/magic-tdx-rs/src/protocol/finance_fields.rs`
- `crates/magic-tdx-rs/src/protocol/types.rs`
- `crates/magic-tdx-rs/src/service/blocks.rs`
- `crates/magic-tdx-rs/src/service/finance.rs`
- `crates/magic-tdx-rs/src/service/funds.rs`
- `crates/magic-tdx-rs/src/service/profile.rs`
- `tools/coverage/check_thresholds.py`
- `tools/coverage/test_check_thresholds.py`
- `tools/coverage/README.md`
- `tools/release/preflight.sh`

### Task 1: Fix Eastmoney exchange identity

**Files:**

- Modify: `crates/magic-eastmoney-rs/src/lib.rs:383-394`
- Test: `crates/magic-eastmoney-rs/src/lib.rs:430-570`

- [ ] **Step 1: Write the failing 9-prefix boundary test**

Add this test beside `code_prefix_must_match_declared_and_source_exchange`:

```rust
#[test]
fn only_verified_920_nine_prefix_maps_to_beijing() {
    let verified =
        InstrumentId::new(Exchange::Beijing, "920118", AssetClass::Equity).unwrap();
    assert!(validate_instrument(&verified).is_ok());

    let unverified =
        InstrumentId::new(Exchange::Beijing, "900901", AssetClass::Equity).unwrap();
    assert!(matches!(
        validate_instrument(&unverified),
        Err(super::EastmoneyError::Unsupported(message))
            if message.contains("unverified 9-prefix")
    ));
    assert!(matches!(
        instrument_from_market("900901", 0),
        Err(super::EastmoneyError::Protocol(message))
            if message.contains("unverified 9-prefix")
    ));
}
```

- [ ] **Step 2: Prove the current implementation fails**

Run:

```bash
cargo test -p magic-eastmoney-rs only_verified_920_nine_prefix_maps_to_beijing --locked --offline
```

Expected: FAIL because `900901` is currently accepted as Beijing.

- [ ] **Step 3: Narrow the verified mapping**

Replace `exchange_for_code` with:

```rust
fn exchange_for_code(code: &str) -> Result<Exchange, String> {
    match code.as_bytes().first().copied() {
        Some(b'6') => Ok(Exchange::Shanghai),
        Some(b'0' | b'3') => Ok(Exchange::Shenzhen),
        Some(b'4' | b'8') => Ok(Exchange::Beijing),
        Some(b'9') if code.starts_with("920") => Ok(Exchange::Beijing),
        Some(b'9') => Err(format!(
            "Eastmoney stock code {code} uses an unverified 9-prefix exchange mapping"
        )),
        Some(prefix) => Err(format!(
            "Eastmoney stock-code prefix {:?} has no verified exchange mapping",
            char::from(prefix)
        )),
        None => Err("Eastmoney stock code is empty".into()),
    }
}
```

- [ ] **Step 4: Run the provider suite**

Run:

```bash
cargo test -p magic-eastmoney-rs --locked --offline
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/magic-eastmoney-rs/src/lib.rs
git commit -m "fix(eastmoney): reject unverified nine-prefix equities"
```

### Task 2: Make TDX zero current prices explicit

**Files:**

- Modify: `crates/magic-tdx-rs/src/adapter.rs:204-286`
- Test: `crates/magic-tdx-rs/src/adapter.rs:1277-1420`

- [ ] **Step 1: Write the failing contextual-error test**

```rust
#[test]
fn zero_current_quote_price_is_an_instrument_contextual_failure() {
    let requested = [instrument("600001")];
    let error =
        normalize_quotes("test", &requested, vec![source_quote("600001", 0.0)]).unwrap_err();
    assert!(matches!(error, TdxError::InvalidData(_)));
    assert!(error.to_string().contains("600001"));
    assert!(error.to_string().contains("current price"));
    assert!(error.to_string().contains('0'));
}
```

- [ ] **Step 2: Prove the current message lacks source context**

Run:

```bash
cargo test -p magic-tdx-rs zero_current_quote_price_is_an_instrument_contextual_failure --locked --offline
```

Expected: FAIL because the current core positivity error does not name the
instrument or source field.

- [ ] **Step 3: Add mandatory source-price validation**

Add:

```rust
fn required_quote_price(
    instrument: &InstrumentId,
    value: f64,
    field: &str,
) -> Result<Price, TdxError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(TdxError::InvalidData(format!(
            "TDX quote {} {field} must be finite and positive, received {value}",
            instrument.code()
        )));
    }
    Price::new(value).map_err(|error| {
        TdxError::InvalidData(format!(
            "TDX quote {} {field} is invalid: {error}",
            instrument.code()
        ))
    })
}
```

Replace the direct `Price::new(record.price)` call with:

```rust
let price = required_quote_price(instrument, record.price, "current price")?;
```

- [ ] **Step 4: Cover negative and non-finite current values**

Extend the test to loop over `[-1.0, f64::NAN, f64::INFINITY]` and require the
same instrument/field context for each value.

- [ ] **Step 5: Run adapter and crate tests**

```bash
cargo test -p magic-tdx-rs adapter::tests --locked --offline
cargo test -p magic-tdx-rs --locked --offline
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/magic-tdx-rs/src/adapter.rs
git commit -m "fix(tdx): reject unavailable current quote prices explicitly"
```

### Task 3: Remove TDX production lock and probe panics

**Files:**

- Create: `crates/magic-tdx-rs/src/sync.rs`
- Modify: `crates/magic-tdx-rs/src/lib.rs`
- Modify: `crates/magic-tdx-rs/src/block/client.rs`
- Modify: `crates/magic-tdx-rs/src/net/client.rs`
- Modify: `crates/magic-tdx-rs/src/net/pool.rs`
- Modify: `crates/magic-tdx-rs/src/net/smart_client.rs`
- Modify: `crates/magic-tdx-rs/src/net/utils.rs`

- [ ] **Step 1: Add failing poisoned-lock tests**

Create `sync.rs` initially with tests referring to the not-yet-created helpers:

```rust
use std::sync::{Mutex, MutexGuard};

use crate::error::TdxError;

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn poisoned() -> Mutex<u8> {
        let mutex = Mutex::new(7);
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _guard = mutex.lock().unwrap();
            panic!("poison test mutex");
        }));
        mutex
    }

    #[test]
    fn fallible_lock_reports_poison_with_context() {
        let error = lock(&poisoned(), "test state").unwrap_err();
        assert!(matches!(error, TdxError::InvalidData(_)));
        assert!(error.to_string().contains("test state"));
        assert!(error.to_string().contains("poisoned"));
    }

    #[test]
    fn compatible_lock_recovers_without_a_second_panic() {
        let mutex = poisoned();
        assert_eq!(*lock_recover(&mutex, "test state"), 7);
    }
}
```

Register `mod sync;` in `lib.rs`.

- [ ] **Step 2: Prove the helpers are absent**

Run:

```bash
cargo test -p magic-tdx-rs sync::tests --locked --offline
```

Expected: compile failure for missing `lock` and `lock_recover`.

- [ ] **Step 3: Implement the two lock policies**

```rust
pub(crate) fn lock<'a, T>(
    mutex: &'a Mutex<T>,
    context: &str,
) -> Result<MutexGuard<'a, T>, TdxError> {
    mutex.lock().map_err(|_| {
        TdxError::InvalidData(format!("TDX {context} mutex is poisoned"))
    })
}

pub(crate) fn lock_recover<'a, T>(
    mutex: &'a Mutex<T>,
    context: &str,
) -> MutexGuard<'a, T> {
    mutex.lock().unwrap_or_else(|poisoned| {
        crate::logw!("sync", "recovering poisoned TDX {} mutex", context);
        poisoned.into_inner()
    })
}
```

- [ ] **Step 4: Migrate every production `Mutex::lock().unwrap()`**

Use `sync::lock(...)?` in `Result` functions and `sync::lock_recover(...)` in
infallible configuration/status/drop/heartbeat paths. Keep test assertions
unchanged. In particular:

- `ConnectionPool::borrow` and `try_borrow` use typed `lock`;
- `TdxBlockClient` data methods use typed `lock`;
- `TdxHqClient::{connect_to_any,connect_internal,try_send_and_recv,
  get_security_list,get_security_count}` use typed `lock`;
- `TdxSmartClient::{connect_to_any,try_next_server}` use typed `lock`;
- pool return/close/stats, rate limiter, configuration setters, disconnect,
  heartbeat, retry helpers, smart health/cache/probe helpers use
  `lock_recover`.

Verify the production search is empty outside tests:

```bash
rg -n '\.lock\(\)\.unwrap\(\)' crates/magic-tdx-rs/src \
  -g '*.rs'
```

Expected: only `#[cfg(test)]` test lines remain.

- [ ] **Step 5: Write a failing probe-session matrix**

In `net/client.rs`, define a private production `ProbeSession` trait, implement
it for a private TCP wrapper, and add a test-only fake implementation whose
`connect`, `handshake`, `send`, `recv header`, and `recv body` stages can fail.
Each case must assert `probe_one` returns `Err`, and a successful case must
return three non-negative latency values.

The shared request/response function used by production and tests is:

```rust
trait ProbeSession {
    fn handshake(&mut self) -> Result<()>;
    fn send(&mut self, packet: &[u8]) -> Result<()>;
    fn recv(&mut self, len: usize) -> Result<Vec<u8>>;
}

fn probe_one<S>(
    connect: impl FnOnce() -> Result<S>,
) -> Result<(f64, f64, f64)>
where
    S: ProbeSession,
{
    let tcp_start = Instant::now();
    let mut session = connect()?;
    let tcp_ms = tcp_start.elapsed().as_secs_f64() * 1000.0;
    let handshake_start = Instant::now();
    session.handshake()?;
    let handshake_ms = handshake_start.elapsed().as_secs_f64() * 1000.0;
    let api_start = Instant::now();
    send_probe_request_and_read_response(&mut session)?;
    let api_ms = api_start.elapsed().as_secs_f64() * 1000.0;
    Ok((tcp_ms, handshake_ms, api_ms))
}
```

Implement `ProbeSession` for a private wrapper around `TcpConnection`.

- [ ] **Step 6: Refactor `probe_servers`**

Call `probe_one` once per server, append only `Ok` results, and sort with:

```rust
results.sort_by(|left, right| left.5.total_cmp(&right.5));
```

Remove `_timeout`, the second connection, and every network `unwrap`.

- [ ] **Step 7: Run targeted and full TDX tests**

```bash
cargo test -p magic-tdx-rs sync::tests --locked --offline
cargo test -p magic-tdx-rs net::client --locked --offline
cargo test -p magic-tdx-rs --locked --offline
```

Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/magic-tdx-rs/src/lib.rs \
  crates/magic-tdx-rs/src/sync.rs \
  crates/magic-tdx-rs/src/block/client.rs \
  crates/magic-tdx-rs/src/net/client.rs \
  crates/magic-tdx-rs/src/net/pool.rs \
  crates/magic-tdx-rs/src/net/smart_client.rs \
  crates/magic-tdx-rs/src/net/utils.rs
git commit -m "fix(tdx): eliminate production synchronization panics"
```

### Task 4: Restore TDX Clippy enforcement

**Files:**

- Modify: all files listed under “Modify for Clippy”
- Modify: `crates/magic-tdx-rs/src/lib.rs`

- [ ] **Step 1: Remove the blanket allowance and capture the real failure**

Delete:

```rust
#![allow(clippy::all)]
```

Run:

```bash
cargo clippy -p magic-tdx-rs --all-targets --locked --offline -- -D warnings
```

Expected: FAIL with the previously measured bounded set of TDX findings.

- [ ] **Step 2: Apply the mechanical corrections**

Apply these exact classes:

| Finding | Correction |
|---|---|
| same-type casts | remove `as u32` / `as i64` |
| manual range checks | use `RangeInclusive::contains` |
| manual multiple checks | use `is_multiple_of` |
| return of a `let` binding | return the expression directly |
| useless `.map_err(Into::into)` | return/await the original `Result` |
| empty line after module docs | move/remove the empty line so `//!` remains attached |
| missing `Default` | delegate `Default::default()` to `new()` |
| field reassignment after default | use struct update syntax |
| `repeat().take()` | use `std::iter::repeat_n` |
| ineffective test multiplication | replace `100 * 1` with `100` |
| complex handshake callback | add `type HandshakeFn = dyn Fn(&mut TcpConnection) -> Result<()> + Send + Sync` |
| `BlockType::from_str` ambiguity | retain the public inherent method and add a narrow item-level `#[allow(clippy::should_implement_trait)]` with a compatibility comment |

For the `single_match`, `match_like_matches_macro`, and `needless_return`
findings, apply Clippy’s equivalent control flow without changing error
handling.

- [ ] **Step 3: Require zero blanket or group allowances**

Run:

```bash
rg -n '#!\[allow\(clippy::all\)\]|allow\(clippy::all\)|allow\(clippy::pedantic\)' \
  crates/magic-tdx-rs
```

Expected: no matches.

- [ ] **Step 4: Run Clippy and tests**

```bash
cargo fmt --all
cargo clippy -p magic-tdx-rs --all-targets --locked --offline -- -D warnings
cargo test -p magic-tdx-rs --locked --offline
```

Expected: all commands pass.

- [ ] **Step 5: Commit**

```bash
git add crates/magic-tdx-rs
git commit -m "fix(tdx): restore workspace clippy enforcement"
```

### Task 5: Implement the documented coverage checker

**Files:**

- Modify: `tools/coverage/check_thresholds.py`
- Modify: `tools/coverage/test_check_thresholds.py`
- Modify: `tools/coverage/README.md`

- [ ] **Step 1: Replace the one-case test with boundary fixtures**

Create helpers that build temporary `crates/<name>/src` trees and llvm-cov JSON.
Add tests named:

```text
test_overall_79_99_fails
test_overall_80_00_passes
test_critical_94_99_fails
test_critical_95_00_passes
test_windows_paths_are_normalized
test_tests_examples_benches_fuzz_and_target_are_excluded
test_existing_unmeasured_critical_family_fails
test_duplicate_file_record_fails
test_malformed_json_fails
test_empty_production_report_fails
```

Use 10,000-line synthetic summaries for exact decimal boundaries.

- [ ] **Step 2: Prove the current checker is incomplete**

Run:

```bash
python3 -m unittest tools.coverage.test_check_thresholds
```

Expected: multiple failures for critical thresholds, path filtering, and
malformed input.

- [ ] **Step 3: Implement strict parsing and selectors**

The implementation must expose:

```python
OVERALL_REQUIRED = 80.0
CRITICAL_REQUIRED = 95.0
CRITICAL_FAMILIES = {
    "codec": lambda path: "/codec/" in path,
    "protocol": lambda path: "/protocol/" in path,
    "adjustment": lambda path: "/adjustment/" in path,
    "service/common.rs": lambda path: path.endswith("/service/common.rs"),
    "adapter.rs": lambda path: path.endswith("/adapter.rs"),
}

def normalized(path: str) -> str:
    return "/" + path.replace("\\", "/").lstrip("/")

def is_production(path: str) -> bool:
    return (
        "/crates/" in path
        and "/src/" in path
        and all(
            marker not in path
            for marker in ("/tests/", "/examples/", "/benches/", "/fuzz/", "/target/")
        )
    )
```

Validate exactly one export object, unique filenames, integer
`covered/count` values satisfying `0 <= covered <= count`, a non-empty
production set, and measured coverage for every critical family that exists
under `repo_root/crates/*/src`.

Print both lines:

```text
overall covered=<n> total=<n> percent=<p> required=80.00
critical covered=<n> total=<n> percent=<p> required=95.00
```

Return `1` below a threshold and raise `SystemExit` with a contextual message
for malformed reports or missing measurements.

- [ ] **Step 4: Run synthetic checker tests**

```bash
python3 -m unittest tools.coverage.test_check_thresholds
```

Expected: all tests pass.

- [ ] **Step 5: Document the exact contract**

Update `tools/coverage/README.md` with the production path rule, combined
critical selectors, equality behavior, missing-family behavior, and exact CI
command.

- [ ] **Step 6: Commit**

```bash
git add tools/coverage
git commit -m "test: enforce critical coverage contract"
```

### Task 6: Raise the combined critical set to 95%

**Files:**

- Modify: `crates/magic-tdx-rs/src/adapter.rs`
- Modify: `crates/magic-tdx-rs/src/protocol/adjuster.rs`
- Modify: `crates/magic-tdx-rs/src/protocol/finance_fields.rs`
- Modify: `crates/magic-tdx-rs/src/protocol/fq_service.rs`
- Modify: `crates/magic-tdx-rs/src/protocol/parsers.rs`
- Modify: `crates/magic-tdx-rs/src/protocol/types.rs`
- Modify: `crates/magic-tdx-rs/src/service/mod.rs`

- [ ] **Step 1: Add the adapter branch matrix**

Add unit tests that exercise all of these explicit branches:

```text
bar empty/source-time paths
compact/display date bad shape and non-digit paths
minute empty, >240, invalid time, duplicate time, negative volume, overflow,
  invalid price, and valid sorted accumulation
optional quote negative/non-finite/zero/positive
quote empty request, duplicate request, cardinality mismatch, duplicate source,
  missing source, unexpected source, invalid current/optional fields,
  invalid volume/amount, complete optional fields, and incomplete fields
trade invalid date shape/digit, empty response/time, invalid price/quantity,
  known/unknown sides, page oversize, short page, and multiple pages
board main/star/chinext/beijing/unknown and ST-name prefix variants
security metadata empty/duplicate request, page mismatch, duplicate source,
  missing source, unexpected source, blank name, and valid partial metadata
book finite/negative/unavailable/present levels, empty/present depth,
  empty/duplicate request, unexpected/duplicate/missing/reordered source
every unsupported P0 trait returns TdxError::Unsupported
```

Use existing `instrument`, `source_quote`, and `source_trade` builders; add only
small field mutations per assertion.

- [ ] **Step 2: Extract and test one order-book normalizer**

Move the repeated TDX/smart/async order-book conversion into:

```rust
pub(crate) fn normalize_order_books(
    source: &str,
    instruments: &[InstrumentId],
    quotes: Vec<SecurityQuote>,
) -> Result<DataBatch<OrderBook>, TdxError>
```

Call it from both adapter implementations and `AsyncTdxService::order_books`.
Test complete levels, zero-tail levels, invalid finite/negative levels,
reordering, missing/duplicate/unexpected identities, and aggregate depth.

- [ ] **Step 3: Add protocol boundary matrices**

Extend existing protocol unit tests with valid and malformed records for each
currently uncovered parser branch:

```text
index bars date/range/truncation
minute average-price and invalid clock fields
transaction coefficient and truncated varints
finance optional-vector lengths
XDXR category/date/value variants
block meta/name/code truncation
adjuster empty events, date boundaries, cash/bonus/rights combinations,
  no-prior-close and invalid factor paths
finance field absent/index/present branches
FQ tier boundary and empty-event paths
default/type constructor branches
```

- [ ] **Step 4: Generate and check the real report**

```bash
cargo llvm-cov --workspace --all-features --json \
  --output-path target/coverage/coverage.json -- --test-threads=1
python3 tools/coverage/check_thresholds.py target/coverage/coverage.json
```

Expected after this task: the printed critical percentage is at least 95.00.
The overall percentage may still be below 80.00 and is completed in Task 7.

- [ ] **Step 5: Commit**

```bash
git add crates/magic-tdx-rs/src/adapter.rs \
  crates/magic-tdx-rs/src/protocol \
  crates/magic-tdx-rs/src/service/mod.rs
git commit -m "test(tdx): cover critical normalization and protocol paths"
```

### Task 7: Raise real workspace coverage to 80%

**Files:**

- Modify: `crates/magic-tdx-rs/src/net/client.rs`
- Modify: `crates/magic-tdx-rs/src/net/async_client.rs`
- Modify: `crates/magic-tdx-rs/src/net/direct_client.rs`
- Modify: `crates/magic-tdx-rs/src/net/finance_client.rs`
- Modify: `crates/magic-tdx-rs/src/net/smart_client.rs`
- Modify: `crates/magic-tdx-rs/src/net/pool.rs`
- Modify: `crates/magic-tdx-rs/src/net/utils.rs`
- Modify: `crates/magic-tdx-rs/src/service/mod.rs`
- Modify: `crates/magic-tdx-rs/src/service/blocks.rs`
- Modify: `crates/magic-tdx-rs/src/service/finance.rs`
- Modify: `crates/magic-tdx-rs/src/service/funds.rs`
- Modify: `crates/magic-tdx-rs/src/service/profile.rs`

- [ ] **Step 1: Add disconnected-client contract matrices**

For sync, async, direct, smart, finance, fund, F10/profile, and service facades,
call every public read operation with valid minimal request arguments while the
client is intentionally disconnected or pointed at an unused loopback port.
Assert the returned variant is a typed connection/disconnected/unsupported
error and never a panic or successful empty record.

The matrix must include:

```text
bars, index bars, quotes, security count/list, minute/history minute,
transactions/history transactions, finance, XDXR, block metadata/data,
fund list/quotes/bars/finance/XDXR, F10 categories/content,
service bars/quotes/trades/books/metadata and all subservice entry points
```

- [ ] **Step 2: Extend the existing async injected-response seam**

Add a test-only constructor inside `async_client.rs` that installs the existing
`spawn_mock_task` sender into a one-slot client pool. Feed valid bodies already
used by parser tests and assert successful public methods for count, list, bars,
quotes, minute, trades, finance, and XDXR. This covers packet construction,
channel request flow, parser dispatch, cache insert/hit, and adapter success
paths without external network.

- [ ] **Step 3: Add a loopback sync protocol server**

Inside `net/client.rs` tests, bind `127.0.0.1:0`, accept a bounded number of
connections, read complete request frames, and return a valid TDX response
header plus a selected fixture body. Cover:

```text
successful handshake and connection
uncompressed and zlib response bodies
cache miss then hit for count/list
retry disabled failure
pool borrow/return/stats/close
disconnect and heartbeat stop
probe success and each typed failure boundary
```

The server thread must have a finite read timeout and be joined by the test.

- [ ] **Step 4: Cover utility and cache state branches**

Add tests for rate-limiter enable/disable/phase transitions, poisoned recovery,
pool exhaustion and failed connection rollback, smart cache expiry/blacklist
load/save/statistics, and China-local clock/date boundaries through existing
deterministic helper functions.

- [ ] **Step 5: Regenerate coverage after each focused batch**

Run:

```bash
cargo llvm-cov --workspace --all-features --json \
  --output-path target/coverage/coverage.json -- --test-threads=1
python3 tools/coverage/check_thresholds.py target/coverage/coverage.json
```

Use the JSON `segments` to list remaining executable zero-count lines. Add a
test only when it represents a distinct observable behavior or failure branch.
Stop only when both printed values satisfy:

```text
overall percent >= 80.00
critical percent >= 95.00
```

Do not add exclusions, `coverage(off)`, ignored tests, external network calls,
or threshold changes.

- [ ] **Step 6: Commit**

```bash
git add crates/magic-tdx-rs
git commit -m "test(tdx): close production coverage gaps"
```

### Task 8: Compile release targets in preflight

**Files:**

- Modify: `tools/release/preflight.sh:17-31`

- [ ] **Step 1: Add a structural failing test**

Run:

```bash
rg -n 'cargo build --workspace --all-targets --release --locked --offline' \
  tools/release/preflight.sh
```

Expected: no match.

- [ ] **Step 2: Add release compilation**

Immediately after the debug `cargo check`, add:

```bash
CARGO_TARGET_DIR="$preflight_target_dir" \
  cargo build --workspace --all-targets --release --locked --offline
```

- [ ] **Step 3: Validate syntax and execute the release build**

```bash
bash -n tools/release/preflight.sh
cargo build --workspace --all-targets --release --locked --offline
```

Expected: both commands pass.

- [ ] **Step 4: Commit**

```bash
git add tools/release/preflight.sh
git commit -m "build: compile release targets during preflight"
```

### Task 9: Run all release gates and review

**Files:**

- Modify as needed only when a gate exposes a regression in an already changed
  file.
- Update: `.planning/2026-07-25-audit-correctness-gates/{task_plan,progress,findings}.md`

- [ ] **Step 1: Run formatting**

```bash
cargo fmt --all -- --check
```

Expected: exit 0.

- [ ] **Step 2: Run workspace build and tests**

```bash
cargo check --workspace --all-targets --locked --offline
cargo test --workspace --all-targets --locked --offline
cargo test --workspace --doc --locked --offline
```

Expected: every command exits 0.

- [ ] **Step 3: Run Clippy and documentation**

```bash
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked --offline
bash tools/docs/check_links.sh
```

Expected: every command exits 0.

- [ ] **Step 4: Run compliance and coverage**

```bash
bash tools/compliance/check.sh
cargo llvm-cov --workspace --all-features --json \
  --output-path target/coverage/coverage.json -- --test-threads=1
python3 tools/coverage/check_thresholds.py target/coverage/coverage.json
```

Expected: compliance passes, overall coverage is at least 80.00%, and combined
critical coverage is at least 95.00%.

- [ ] **Step 5: Run release build and full preflight**

```bash
cargo build --workspace --all-targets --release --locked --offline
bash tools/release/preflight.sh
```

Expected: both commands exit 0.

- [ ] **Step 6: Review repository invariants**

```bash
rg -n '#!\[allow\(clippy::all\)\]|\.lock\(\)\.unwrap\(\)' \
  crates/magic-tdx-rs/src -g '*.rs'
git diff --check
git status --short
git log --oneline --decorate -12
```

Expected: no blanket Clippy allowance, no production lock unwrap outside test
code, no whitespace errors, and only intentional branch commits.

- [ ] **Step 7: Commit final evidence**

```bash
git add .planning/2026-07-25-audit-correctness-gates
git commit -m "chore: record audit remediation evidence"
```
