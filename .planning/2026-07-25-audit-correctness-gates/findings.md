# Findings & Decisions

## Requirements

- Fix the issues that were verified against the current repository.
- Preserve explicit failures and source provenance.
- Do not add downstream path dependencies.
- Run formatting, tests, Clippy, compliance, and documentation checks before
  release.
- Make technical choices autonomously after the approved design.

## Research Findings

- `crates/magic-eastmoney-rs/src/lib.rs` currently maps every six-digit code
  beginning with `9` to Beijing. This deterministically misclassifies Shanghai
  B-share code `900901`.
- Other verified providers already use the narrower rule: `4`, `8`, and `920`
  map to Beijing; other `9` prefixes are rejected.
- `normalize_quotes` in `crates/magic-tdx-rs/src/adapter.rs` passes the mandatory
  current price directly to `Price::new`. Zero therefore produces a generic
  positivity error with no instrument/field context, while optional OHLC fields
  already preserve zero as absence.
- `TdxHqClient::probe_servers` performs a second connection, handshake, request,
  response read, and parse through `unwrap`, even though the first probe is
  fallible. A transient failure in the second probe can panic the process.
- TDX production code contains numerous `Mutex::lock().unwrap()` calls. Test
  unwraps and examples are not production defects and are not part of this
  remediation.
- `crates/magic-tdx-rs/src/lib.rs` has `#![allow(clippy::all)]`; the current green
  Clippy result does not inspect that crate under the workspace `all = deny`
  policy.
- `tools/coverage/check_thresholds.py` implements only the overall 80% test and
  has one boundary test. The committed release plan also requires production
  path filtering, a 95% aggregate for critical paths, missing-path failure,
  Windows path handling, and malformed-report failure.
- The exact scheduled-CI coverage command currently reports 26,370 of 36,682
  lines, or 71.89%.
- The historical critical set currently reports 1,780 of 2,721 lines, or
  65.42%. The largest gaps are TDX `adapter.rs` and `protocol` modules.
- `tools/release/preflight.sh` checks debug targets, tests, Clippy, rustdoc,
  links, and compliance, but does not compile all release targets.
- `TdxError` has no synchronization-specific variant; `InvalidData` is the
  existing non-breaking carrier for internal state that cannot be trusted.
- TDX already exports a `logw!` macro controlled by `TDXRS_LOG`, so compatible
  infallible lock recovery can emit an observable warning without adding a
  logging dependency.
- `RateLimiter` exposes infallible `wait`, setter, and getter methods. Its lock
  handling must therefore use warning-backed recovery unless the public API is
  deliberately broken.
- `ConnectionPool::borrow` and `try_borrow` are fallible and can return a typed
  poison error. `push`, `return_connection`, `close_all`, and `stats` are
  compatibility-bound infallible paths and need warning-backed recovery.
- `TdxBlockClient` has infallible server/timeout setters but fallible data
  methods. Its mutex can therefore use recovery for setters and typed failure
  for all data requests without changing signatures.
- `TdxHqClient` mixes fallible connection/data operations with infallible
  configuration, status, disconnect, heartbeat, and retry helpers. The shared
  synchronization utility must expose both typed `lock` and warning-backed
  `lock_recover` forms.
- Heartbeat runs in a spawned thread and currently locks `last_server` with
  `unwrap`; a poisoned lock would kill only the heartbeat thread and silently
  disable liveness management. This path needs recovery plus a warning.
- `connect_to_any`, `connect_internal`, `try_send_and_recv`, and cache-backed
  request methods already return `Result` and can propagate lock poisoning
  explicitly.
- `TdxSmartClient::connect_to_any` and `try_next_server` are fallible, while
  `lazy_health_check`, cache reporting/reset, and probe caching are infallible
  compatibility paths. The same two lock helpers cover this module cleanly.
- TDX cache accesses in `get_security_list` and `get_security_count` are already
  inside `Result` APIs and should fail explicitly instead of recovering possibly
  inconsistent cached state.
- The largest overall coverage gaps are concentrated in TDX: synchronous client
  (859 uncovered lines), adapter (618), async client (447), service facade
  (400), finance client (364), direct client (346), and smart client (231).
- The llvm-cov JSON includes per-file `segments`, so focused tests can be driven
  by exact uncovered executable lines rather than by raw source line counts.
- Most uncovered TDX adapter lines are concrete sync/direct/smart/async trait
  wrappers after successful network calls, not only pure normalization
  branches. Reaching the documented critical threshold requires deterministic
  successful transport tests rather than superficial invalid-input calls.
- `TcpConnection` wraps a normal `TcpStream`; an in-process loopback protocol
  server can exercise production send/receive paths without adding a production
  transport abstraction or external network dependency.
- `--force-warn clippy::all` exposes 64 TDX library findings and five additional
  test findings. They are bounded and mostly mechanical: same-type casts,
  `Default` implementations, range checks, useless conversions, and simple
  control-flow cleanup. The only likely narrow policy exception is the complex
  handshake callback type, which can instead be fixed with a type alias.
- Async TDX tests already inject responses through a private `mpsc::Sender<Request>`
  seam. Extending that pattern is preferable to a new production transport
  abstraction for adapter and service coverage.
- Order-book normalization is duplicated between `adapter.rs` and
  `service/mod.rs`. Extracting one provider-neutral private normalizer makes the
  behavior directly testable and removes hundreds of hard-to-cover duplicate
  branch lines without changing the public contract.
- The final exact report reaches 30520/38150 production lines (80.00%) overall
  and 3480/3663 configured critical lines (95.00%).
- The reported absence of cargo-deny enforcement is false for the current
  repository: both `.github/workflows/ci.yml` and
  `.github/workflows/security.yml` run the pinned cargo-deny action. Only the
  local developer binary is absent.
- Compiling all workspace release targets exposes pre-existing Cargo output
  filename-collision warnings because many provider examples share
  `live_probe` and `load_probe` names. This remains non-fatal today but should
  be resolved as a separate example-compatibility cleanup.

## Technical Decisions

| Decision | Rationale |
|----------|-----------|
| Reject zero TDX current price atomically with instrument and raw-field context | Quote batches are cardinality-checked and mandatory current price cannot truthfully be represented as `None`. |
| Omit a server from `probe_servers` if either probe phase fails | This preserves the existing return contract while eliminating process panic. |
| Use `f64::total_cmp` for latency ordering | Probe latency is finite by construction, and total ordering removes the remaining comparison unwrap. |
| Propagate lock poison through `TdxError` in fallible paths | This preserves explicit failure semantics. |
| Recover with a warning only for compatibility-bound infallible paths | This avoids a public API break while ensuring poison is not silently swallowed or cascaded into another panic. |
| Add tests rather than exclusions to satisfy coverage | Production omissions must stay visible. |

## Issues Encountered

| Issue | Resolution |
|-------|------------|
| The real report already fails the existing 80% checker | Treat the red gate as existing debt and add focused behavior tests until it is genuinely green. |
| The main checkout has unrelated dirty/untracked files | Created `.worktrees/audit-correctness-gates` on `fix/audit-correctness-gates`. |

## Resources

- `docs/superpowers/plans/2026-07-21-magic-market-data-rs-phase-5-release.md`
- `crates/magic-eastmoney-rs/src/lib.rs`
- `crates/magic-tdx-rs/src/adapter.rs`
- `crates/magic-tdx-rs/src/net/client.rs`
- `tools/coverage/check_thresholds.py`
- `tools/release/preflight.sh`

## Visual/Browser Findings

- No browser or visual research was required.
