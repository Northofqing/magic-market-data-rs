# Findings & Decisions

## Requirements
- Release the synchronous TDX outer pool-handle lock before socket I/O.
- Prove synchronous pool concurrency with a deterministic loopback regression.
- Document that HTTP providers are blocking and show Tokio `spawn_blocking`.
- Correct `CONTRIBUTING.md` rolling-stable wording.
- Register and mechanically enforce current direct/shared/hybrid HTTP backends so no new unreviewed local stack is introduced.
- Preserve typed failures, provenance, admission behavior, and existing public routing semantics.

## Research Findings
- The architecture audit and repository inspection established the root cause at `TdxHqClient::try_send_and_recv`: a `MutexGuard<Arc<ConnectionPool>>` remains live because `PooledConnGuard` borrows through it across send/receive.
- BR-003 promises five synchronous connections, so outer-lock serialization contradicts the registered pool policy.
- Provider HTTP stacks are split across 14 direct ureq crates, 10 shared-transport-only provider crates, and one hybrid exchange crate.
- Public documentation does not state the Tokio `spawn_blocking` requirement, while `CONTRIBUTING.md` incorrectly describes a pinned minimum toolchain.
- Recent history is audit/release-profile hardening; this slice must preserve those fail-closed benchmark and evidence changes.
- Existing `client.rs` tests already replace the private pool and last-server state, so the concurrency regression can remain a private deterministic unit test.
- A bounded loopback reproduction can accept the first request, observe whether a second connection/request arrives before replying to the first, then always reply to both. It fails on the current outer-lock lifetime without hanging and passes once the pool `Arc` is cloned before borrow.
- Existing compliance unit tests import their checker module directly from its
  filesystem path and construct temporary tracked Git repositories. The HTTP
  transport checker can follow that convention to cover valid registries,
  missing/duplicate rows, dependency drift, malformed values, untracked files,
  and symlink rejection without network access.
- `tools/compliance/check.sh` already invokes the admissions checker as an
  independent deterministic gate. The HTTP checker should be invoked beside it,
  with the transport registry and blocking-integration guide added to the
  required-file list.
- Manifest inventory confirms 14 provider crates with direct `ureq`, ten
  shared-transport-only provider crates, one hybrid (`magic-exchange-rs`), and
  the `magic-market-transport` infrastructure crate with direct
  `reqwest`/`rustls`. `magic-eastmoney-rs` additionally declares direct
  `ring`.
- `CONTRIBUTING.md` currently calls the default-toolchain preflight a "pinned
  minimum toolchain" run, contradicting the repository's rolling-stable
  policy. The integration index only documents admissions, and the root README
  has no warning that the HTTP provider clients are blocking.
- The TDX request parser rejects an empty response body, so the loopback
  regression must return a 16-byte response header with both encoded and
  decoded sizes set to one, followed by one body byte.
- A test client must set `connected=true`, disable retry and all three rate
  limiters, replace `last_server`, and install a handshake-free `PoolConfig`
  with `max_size=2`. A start barrier makes both caller threads contend while
  the loopback server withholds the first response until it observes the second
  request or reaches a bounded deadline.
- The HTTP checker can use Python's standard-library `tomllib`, restrict
  discovery to production `[dependencies]`, and compare the normalized
  dependency set plus shared-transport presence against the tracked TSV.
  Feature names such as `ureq/native-tls` are not direct dependencies and must
  not be misclassified.
- The blocking guide can use a compiling Tencent-shaped example with
  `TencentClient: Clone`, `InstrumentId::new(Exchange::Shanghai, "600000",
  AssetClass::Equity)`, and the `RealtimeQuotes::realtime_quotes` method. The
  Tokio task must move both the cloned client and owned instrument vector, then
  apply `await?` for `JoinError` separately from the provider `Result`.

## Technical Decisions
| Decision | Rationale |
|----------|-----------|
| Clone the pool `Arc` under the outer mutex and borrow after releasing it | Minimal root-cause fix; connection-pool lifecycle semantics stay inside `ConnectionPool` |
| Use a tracked TSV registry plus compliance checker for HTTP backends | Auditable, deterministic, and consistent with the existing admissions registry pattern |
| Keep live probes outside preflight | Preflight is intentionally locked/offline and must not mutate admission evidence |

## Issues Encountered
| Issue | Resolution |
|-------|------------|
| The original request spans multiple independent architecture programs | Limit this spec to the bounded P0 slice and leave later work for separate Gate A specs |
| Initial findings-log patch used stale section wording | Re-read the active planning files and applied a narrow patch |
| The approved spec described an empty TDX response body, but the existing client explicitly rejects empty bodies | Corrected the spec and plan to use a valid one-byte body without changing the approved architecture |
| The repository is a normal `main` checkout rather than a linked worktree | Treat the user's explicit “继续干” after the plan was committed on `main` as authorization to execute inline in place; preserve and never stage planning-session state |
| The first red-test server expected a serialized second request to open a new socket, but the pool correctly reuses the first idle socket after response completion | In the non-concurrent fallback, read and answer the second request on the first accepted stream so the old implementation reaches only the intended concurrency assertion |
| On macOS, streams accepted from the nonblocking loopback listener inherited nonblocking mode, so `read_exact` returned `WouldBlock` despite a read timeout | Explicitly restore blocking mode on each accepted stream before bounded reads |
| `Arc::clone(&MutexGuard<Arc<_>>)` does not apply the required deref coercion | Clone from `&*guard`; the temporary guard still ends at the statement boundary |
| The repository link checker scans Markdown links inside fenced implementation-plan examples | Use repository-root `docs/integrations/...` targets in plan snippets so both the planned content and the plan artifact pass the deterministic checker |
| Cargo accepts nested members, target-specific dependencies, renamed dependencies, and implicit in-tree path-dependency members | Derive declared members from the root workspace, recursively traverse in-repository path dependencies while honoring `exclude`, and normalize aliases through `package` |
| `csv.DictReader` can silently retain extra fields under `None` and emit `None` for missing fields | Reject row-shape drift before semantic validation and convert Unicode/CSV/I/O failures to stable diagnostics |

## Resources
- `docs/business_rules.md` BR-003, BR-009, BR-010, BR-029
- `docs/superpowers/specs/2026-07-21-magic-tdx-rs-design.md`
- `docs/superpowers/specs/2026-07-23-unpinned-rust-toolchain-design.md`
