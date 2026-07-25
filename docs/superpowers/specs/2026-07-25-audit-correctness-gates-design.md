# Audit Correctness and Release Gates Design

## Objective

Correct the verified Eastmoney and TDX defects and restore truthful release
enforcement. The result must preserve explicit failures and provenance, keep
existing public APIs compatible, compile all release targets, and pass the
documented coverage thresholds without excluding production code or lowering
the thresholds.

## Scope

This change contains six bounded components:

1. Eastmoney A-share exchange identity validation.
2. TDX mandatory current-price normalization.
3. TDX network-probe and synchronization panic safety.
4. TDX Clippy enforcement.
5. Workspace coverage enforcement and focused coverage tests.
6. Release-target compilation in the preflight gate.

Dynamic provider registration, shared HTTP transport extraction, an asynchronous
router, and shell configuration outside this repository are separate
architecture or environment tasks and are not part of this design.

## Eastmoney Exchange Identity

The common Eastmoney code validator will use only verified mappings:

- `6xxxxx` is Shanghai;
- `0xxxxx` and `3xxxxx` are Shenzhen;
- `4xxxxx`, `8xxxxx`, and `920xxx` are Beijing;
- every other prefix, including unverified `9xxxxx` codes such as `900901`, is
  rejected before transport or rejected as source protocol data.

All callers continue to use the same validation function, so request validation,
source identity validation, `secid`, and market-number checks cannot diverge.
Regression tests cover accepted `920` identities, rejected `900901` identities,
and declared/source exchange mismatches.

## TDX Quote Semantics

The normalized `Quote` contract requires a positive current `Price`. A zero TDX
packet value can mean suspended, untraded, or unavailable, but the packet does
not prove which interpretation applies. The adapter therefore will not:

- substitute the previous close;
- emit an available quote with price zero;
- change the core `Price` type to admit zero; or
- silently drop only the affected row from an atomic request batch.

Instead, normalization returns `TdxError::InvalidData` naming the instrument,
the `current price` field, and the raw zero value. Non-finite and negative values
receive the same source-contextual validation. Optional previous close and OHLC
fields keep their current zero-as-absence behavior.

## TDX Panic Safety

### Server probing

Both TCP/handshake and API-latency measurements will execute through fallible
helpers. If either phase fails for a server, that server is omitted from the
returned probe list, matching the current API contract. No transient network or
parse failure may panic the process. Results use `f64::total_cmp`, eliminating
the latency comparison unwrap.

The network measurement helper will accept the connection operation through an
internal seam so deterministic tests can prove that second-phase connect, send,
receive, header-parse, and body-read failures are skipped rather than panicking.
Production still uses the real `TcpConnection`.

### Synchronization

Production `Mutex::lock().unwrap()` calls in TDX are classified by their caller:

- fallible operations map poison to a contextual `TdxError`;
- infallible compatibility methods use one audited recovery helper that emits a
  TDX warning before taking the poisoned inner value;
- tests and documentation examples may continue using `unwrap` where a failure
  is the test assertion mechanism.

This removes cascading production panics without silently erasing the poison
signal and without changing established public method signatures.

## Clippy Enforcement

The crate-level `#![allow(clippy::all)]` is removed. The TDX crate must pass the
workspace `all = deny` policy across all targets.

Findings are handled in this order:

1. fix correctness, needless allocation, ownership, and control-flow findings;
2. preserve an imported protocol shape only when changing it would obscure the
   verified wire format;
3. for such a justified case, add the smallest named lint allowance at the
   narrowest item or module with a reason comment.

No lint group or crate-wide blanket allowance is permitted.

## Coverage Contract

The checker will parse llvm-cov JSON deterministically and include only
production files under `crates/*/src`. It normalizes Windows separators and
excludes tests, examples, benches, fuzz targets, and generated target output.

It enforces:

- at least 80% aggregate production line coverage; and
- at least 95% aggregate coverage across the configured critical families:
  `codec/`, `protocol/`, `adjustment/`, `service/common.rs`, and `adapter.rs`.

A configured family contributes when that family exists in the repository. If
an existing configured family has no measured file in the report, the checker
fails instead of silently shrinking the critical set.
Malformed reports, missing data arrays, non-numeric summaries, empty production
sets, below-threshold totals, and duplicate file records fail explicitly.
Threshold equality passes.

The current exact CI baseline is 71.89% overall and 65.42% for the combined
historical critical set. The implementation therefore includes focused behavior
tests until the real report, not only synthetic checker fixtures, reaches both
thresholds. No production path may be excluded merely to improve the number.

## Release Preflight

`tools/release/preflight.sh` will compile:

```text
cargo build --workspace --all-targets --release --locked --offline
```

inside its temporary target directory. Existing format, check, test, Clippy,
rustdoc, documentation-link, compliance, and diff checks remain intact.
Coverage generation stays in the scheduled security workflow because it is a
separate instrumented build, but the final verification for this change runs it
explicitly.

## Testing Strategy

Tests are written before each behavior change:

- Eastmoney tests demonstrate the current `900901` misclassification and the
  intended `920` boundary.
- TDX adapter tests demonstrate generic zero-price failure and then require
  instrument/field/value context.
- TDX probe tests inject failures at every second-phase boundary and assert no
  panic and no admitted server.
- TDX synchronization tests deliberately poison representative state and assert
  typed failure or warning-backed recovery according to the API category.
- Coverage-checker tests cover 79.99/80.00, 94.99/95.00, path normalization,
  exclusions, absent measured critical families, duplicate rows, and malformed
  JSON.
- Preflight is syntax-checked and the exact release build command is executed.

Final verification runs formatting, all workspace targets, Clippy with warnings
denied, rustdoc and doc tests, link checks, compliance, the real coverage report,
release compilation, and `git diff --check`.

## Acceptance Criteria

- Eastmoney never identifies `900901` as Beijing and still accepts verified
  `920xxx` Beijing equities.
- TDX zero current prices fail atomically with source-contextual diagnostics and
  are never fabricated.
- The identified TDX production network and lock paths do not panic on their
  fallible inputs.
- TDX has no crate-wide Clippy suppression and the workspace Clippy gate passes.
- Real coverage is at least 80% overall and 95% across the existing configured
  critical families.
- Preflight compiles all release targets.
- All repository release checks pass from the isolated branch.
