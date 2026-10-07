# gRPC upstream boundary coverage

Date: 2026-10-07

## Gate A scope and existing seam

Continue the verified standing repair and source integration authorization at
the already agreed `OperationRegistry::execute` seam from the handler-behavior
coverage design. Use the real existing Tencent and Sina registration factories
and public transport injection. No new interface, registration factory,
architecture, provider admission, network policy or runtime authority is added.

The original exact-head CI for 2b225b71148425d18c82a3dcc552f1444f3595cd,
run 37564892679, failed at critical 35411/39819 (88.93%, required 95%).
Overall 69340/81600 passed 80%; audit, artifact production and upload passed.
This failure, original JSON and checker remain immutable. grpc_production.rs
has 1864 uncovered measured lines; derived_products.rs has 230. Missing lines
are prioritization evidence, not proof of defects or a predicted coverage gain.

## Behavior slices

Add an external test module through the existing external handler test module,
preserving all old test bodies and the complete old file prefix. Exercise one
behavior and run it before adding the next. Transport fixtures terminate at the
external API boundary; the real SDK parser, registered handler and envelope
projection remain in the execution path. Pin the selected Provider so another
registered production client cannot be queried.

- Intraday shape retains independently worked price, direction, volume and VWAP
  values; absent source amounts remain absent. Mixed amount availability,
  requested point limits, stale current-session dates, post-market-only windows
  and malformed source rows fail atomically with their real categories.
- Tencent bars, minute data, trades, books and statistics retain source identity,
  request bounds, quality and payload ceilings through the registered handlers.
  Unsupported scopes and invalid input fail before transport when the SDK
  contract requires it; source errors never become complete empty results.
- Sina parity handlers retain their actual normalized records and distinguish
  incomplete source evidence from an acquisition failure. Fixtures are offline
  source examples, not admission witnesses or successful live probes.

Use literal independent expected results, not a second parser or snapshots
calculated by the implementation. Invocation witnesses establish pre-transport
refusal only at the injected external transport. Do not infer network purity
for unrelated existing workspace tests. A newly discovered production defect
requires an explicit reproducer and separately justified minimal repair.

## Gates B through D

No production function body changes are planned. Do not modify the checker,
80/95 thresholds, globs, source placement, ignores, dependencies, credentials,
allowlists or transports. New tests that pass characterize existing behavior;
they are not reported as a production bug fix or as measured release coverage.

Run focused, affected-library and workspace tests, formatting, strict Clippy,
Rustdoc, compliance and the original documentation checker. Retain all failures
and verify the old tests and user notes unchanged. Review Standards and Spec
independently at a fixed base and final source. Source publication must follow
the existing verified nonforce authorization and recheck current main ancestry.
Observe one original exact-head CI; do not repeat unchanged workflow dispatches.

The 2418-line deficit assumes the current denominator remains unchanged. Only
the original new-head CI can establish 95% qualification. No SDK release tuple,
new business RPC or formal service switch is implied by local test results.
Old Windows and Mac runtime artifacts remain untouched. Rollback removes only
the new test module, its declaration and this design, preserving old evidence.
