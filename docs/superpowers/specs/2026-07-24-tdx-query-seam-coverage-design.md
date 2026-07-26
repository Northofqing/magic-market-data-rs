# TDX decoded-query seam and critical-path coverage design

Date: 2026-07-24

Status: Gate A design ready

Related rules: BR-001, BR-002, BR-007

## 1. Problem and release blocker

`magic-tdx-rs` already has strict protocol and parser coverage, but the public
Provider implementations in `adapter.rs` and the facade logic in
`service/mod.rs` call concrete clients directly. Deterministic tests can prove
preflight failures and disconnected-client failures, but they cannot supply a
decoded successful response without opening a real socket.

The current measured evidence is:

- Router: 675/693 lines, 97.40%;
- TDX protocol: 940/977 lines, 96.21%;
- TDX adapter: 757/1067 lines, 70.95%;
- TDX service facade: 199/428 lines, 46.50%.

The configured critical aggregate must be at least 95%. Lowering the threshold,
excluding these production files, moving production logic to an unmeasured
file, adding inline test bodies, or using an uncontrolled live server as the
test fixture is prohibited.

## 2. Decision

Add crate-private decoded-query seams at the adapter and service boundaries.
The seam accepts and returns the same decoded source records that the existing
clients already expose. Production implementations delegate to the same
concrete client methods as today. Deterministic tests provide scripted decoded
records or typed failures.

The seam is not a second transport and does not own sockets, retries,
connection pools, health ordering, heartbeats, rate limits, packet encoding or
parsing. Those remain in the existing clients and protocol modules.

The seam is crate-private. No public type, public method signature, feature,
dependency or downstream integration contract changes.

## 3. Module boundary

`adapter.rs` owns:

- request-to-TDX parameter validation;
- calls through the decoded-query seam;
- strict pagination orchestration;
- response identity and cardinality checks;
- normalization into `magic-market-core` records;
- provenance and quality construction.

`service/mod.rs` owns:

- protocol-sized quote chunking;
- complete security-list pagination;
- facade-specific source labels;
- delegation to the existing Smart and async clients.

The existing protocol parsers remain independently fixture-tested. The seam
must not accept raw bytes and must not duplicate parsing logic.

## 4. Data flow

Production:

```text
Core request
  -> adapter/service validation
  -> crate-private decoded-query trait
  -> existing TdxHqClient / TdxSmartClient / TdxDirectClient /
     AsyncTdxHqClient method
  -> existing packet transport and parser
  -> decoded source record
  -> identity/cardinality/pagination checks
  -> normalized Core batch with provenance
```

Deterministic test:

```text
Core request
  -> same adapter/service validation
  -> scripted decoded-query implementation
  -> decoded fixture record or typed TdxError
  -> same identity/cardinality/pagination checks
  -> same normalized Core batch with provenance
```

Tests may not return an implicit empty success. An unconfigured scripted call
is a typed test failure.

## 5. Interface shape

The blocking seam covers bars, quotes, security count/list, minute data and
current/historical transactions. The async seam covers the corresponding
operations used by the async facade.

Private generic helpers take `&impl BlockingTdxQuery` or
`&impl AsyncTdxQuery` and contain the existing orchestration and normalization.
Public trait implementations remain thin delegates to these helpers.

Calls from production seam implementations use fully qualified inherent client
methods where a trait method has the same name. This prevents accidental
recursion.

The following behavior remains unchanged:

- `TdxHqClient` uses its existing blocking methods;
- Smart bars and quotes continue through Smart failover;
- Smart metadata/minute/trades and `TdxService` list/chunk operations continue
  through the same current inner-client path;
- Direct implementations continue through the Direct client;
- async implementations continue through the existing async pool;
- every existing source label and batch identity remains exact.

## 6. Consolidation

The three order-book implementations currently repeat five-level
normalization. Extract one private normalization function that accepts ordered
decoded quotes and an exact source label. Hq, Smart and async callers reuse it.

This is a behavior-preserving consolidation, not a new fallback. It must keep:

- five fixed bid and ask levels;
- atomic price/quantity presence;
- non-finite and negative-value rejection;
- zero-price unavailable levels;
- exact total-depth calculation;
- missing, duplicate and unexpected instrument rejection;
- explicit unavailable source time and associated quality issue.

## 7. Failure modes

| Failure | Required outcome |
| --- | --- |
| invalid/empty request | typed `InvalidData` before query |
| Beijing mapping where currently unsupported | preserve existing behavior |
| query transport/protocol error | return the original typed error |
| quote response missing/duplicate/unexpected | reject the entire batch |
| paginated middle-page failure | reject the entire batch |
| empty or oversized middle page | reject the entire batch |
| declared security count mismatch | reject the entire batch |
| non-finite/negative order-book value | reject the entire batch |
| incomplete book level | preserve explicit unavailable quality |
| provenance/source label drift | golden test failure |
| scripted test call not configured | typed test failure, never empty success |

No branch may fabricate missing values, source times, records or successful
quality.

## 8. Test matrix

External path-based tests under `tests/internal/` cover:

1. blocking, Smart, Direct and async bars success plus range/empty failures;
2. quotes success, response reordering, duplicate, missing and unexpected
   identities;
3. current and historical trade pagination, short terminal page, oversized
   page and middle-page error;
4. current and historical minute success and invalid source values;
5. full and partial five-level books, non-finite/negative values and response
   identity failures;
6. security counts of zero, one, 1000, 2001 and count/page mismatches;
7. quote requests above 60 split into protocol-sized chunks and abort
   atomically on any failed/incomplete chunk;
8. exact source/provider/batch provenance for every facade.

Production source files may contain only path-based `#[cfg(test)] mod tests;`
declarations, never inline test bodies.

## 9. Coverage evidence

The implementation is acceptable only after a fresh workspace report proves:

```bash
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo llvm-cov \
  --workspace --all-features --locked --offline \
  --json --output-path target/coverage/coverage.json \
  -- --test-threads=1

python3 tools/coverage/check_thresholds.py target/coverage/coverage.json
```

The checker must report overall production coverage at least 80%, configured
critical aggregate at least 95%, and every configured critical glob measured.
Estimates and narrow crate reports are not Gate D evidence.

If another configured critical module is not compiled, it must be resolved by
an architecture decision based on whether it is admitted production code. It
must not be reconnected only to inflate coverage and must not be silently
removed from the gate.

## 10. Rollout and rollback

Rollout:

1. commit this design independently;
2. add failure-first external tests;
3. add the crate-private seam and production delegates;
4. consolidate order-book normalization;
5. run targeted tests, full workspace gates and fresh coverage;
6. retain the change only if behavior/provenance golden tests and all release
   gates pass.

Rollback is a single implementation-commit revert. Because public API and
serialized contracts do not change, downstream callers require no rollback.

## 11. Old module relation

| Existing module | Decision | Reason |
| --- | --- | --- |
| concrete TDX clients | adopt unchanged | retain real transport and failover |
| protocol parsers | adopt unchanged | already independently fixture-tested |
| adapter pure normalization helpers | adopt and deepen | one checked boundary |
| repeated order-book normalization | replace | eliminate divergent copies |
| live server in unit tests | reject | nondeterministic and unsafe evidence |
| coverage exclusions/threshold reduction | reject | violates release contract |

