# Audit Hardening Design

**Status:** Approved in conversation on 2026-07-29

## Objective

Eliminate the confirmed correctness and contention defects found in the
2026-07-29 audit while preserving explicit failures, source provenance,
provider-neutral routing, and Gates A through D.

The completed change must:

- reject every truncated TDX packet atomically instead of producing zero-valued
  or partial market records;
- stop Exchange request pacing from holding its reservation lock during sleep
  and complete network I/O;
- detect drift between production capability constants and BR-009 admission
  evidence;
- centralize fixed-offset timestamp conversion and reusable floating-point
  comparison mechanics without erasing business units;
- change the workspace release profile only when a repeatable benchmark proves
  a material benefit.

## Non-goals

- Replacing all checked `f64` domain values with decimal or fixed-point types.
- Turning unadmitted NBS, FRED, IMF, World Bank, SEC, PBC social-financing, or
  CFETS DR007 diagnostics into production capabilities without new live
  evidence.
- Adding concrete Provider dependencies to `magic-market-router`.
- Converting every synchronous Provider into an asynchronous client.
- Treating public access as permission to mirror or redistribute source data.

## Design Principles

1. A declared nonempty response is indivisible: either every declared record is
   validated or the entire parse fails.
2. Missing and zero are distinct source states. Decoder failure is neither.
3. Shared infrastructure may centralize mechanics but cannot merge different
   source identities, units, TLS evidence, or authorization boundaries.
4. A performance claim requires reproducible measurements from the exact
   candidate revision.
5. Pre-1.0 API hardening is allowed when retaining an infallible API would
   preserve a data-corruption or panic path; the change must be documented.

## 1. Strict TDX Binary Parsing

### Checked cursor

`magic-tdx-rs` will introduce one checked packet cursor that owns:

- the immutable input slice;
- the current byte offset;
- fixed-width little-endian reads;
- bounded byte-slice reads;
- signed TDX variable-integer decoding;
- field and record context for typed length errors.

Every cursor operation returns `Result<_, TdxError>`. A failed read reports
`RESPONSE_LENGTH_MISMATCH` with the byte offset, requested width, field, and
record index where applicable. Variable integers have a fixed maximum encoded
width and reject an unterminated continuation sequence.

The public low-level helpers in `constants.rs` and `helpers.rs` become fallible
instead of panicking or returning zero. This is an intentional 0.x safety
change. All workspace callers and examples are updated in the same commit, and
the change is recorded in the changelog and TDX README.

### Atomic parser contracts

Server response parsers must:

- validate the complete fixed header before reading its declared count;
- parse exactly that many records;
- reject an incomplete record rather than `break`;
- reject a truncated fixed or variable field;
- verify parser-specific tail semantics after the declared records.

No generic tail rule is imposed where the wire contract documents padding or a
separate payload. Such padding must be named and validated explicitly.

Specific corrections include:

- security list and realtime quote parsers no longer return a shorter vector
  than the declared count;
- security/index bars use checked variable fields followed by checked volume
  and amount fields;
- realtime and historical minute parsers reject a partial price/auxiliary/
  volume tuple;
- historical transactions use the same checked variable decoder as current
  transactions;
- fixed-record local readers use checked reads even after their record-size
  preflight, preventing future call-site regressions.

### Tests

Each parser receives:

- one valid minimum packet;
- truncation at every byte boundary of that packet;
- declared-count truncation after a valid earlier record;
- unterminated maximum-width variable integers;
- valid zero fields that remain distinguishable from decoder failure;
- trailing-byte tests matching its exact protocol contract.

Fuzz smoke tests continue to prove absence of panic and additionally require
each arbitrary input to return either a fully self-consistent record set or a
typed error.

## 2. Exchange Transport and Request Pacing

### Shared contracts

`magic-exchange-rs` will depend on `magic-market-transport` for:

- request-start reservations;
- endpoint allowlist and query-key validation;
- bounded response and media-type validation;
- shared request/response value objects where their contract is compatible.

The shared `RequestGate` reserves a start instant under a short mutex, releases
the mutex, waits, and returns before the caller begins I/O. Clones therefore
remain spaced without being serialized for an entire slow response.

### TLS boundary

Exchange currently exposes explicit Rustls and native-tls operator evidence.
That contract will not be silently collapsed into the shared Rustls-only
executor.

The Exchange wire adapter may remain source-specific for the actual TLS call
until `magic-market-transport` supports the same explicit backend choice.
Regardless of backend, every production request passes the shared policy
validation before I/O and shared response validation after I/O. Injected
fixture transports remain available.

The production gate no longer holds a mutex through sleep or network I/O.
Load probes record actual request-start gaps and maximum concurrent requests so
the changed semantics are visible rather than inferred.

## 3. Machine-checked BR-009 Admission Registry

A tracked, machine-readable admission registry will record one row per
capability constant with:

- Provider crate and Provider identity;
- exact Rust capability constant;
- admitted boolean;
- integration evidence document;
- admission status;
- last live-evidence date;
- consecutive live-probe count;
- serial-load count;
- explicit blocker for every false capability.

`tools/compliance/check.sh` will reject:

- an admission constant absent from the registry;
- a registry boolean that differs from the Rust constant;
- an admitted row without a tracked evidence document;
- an admitted row without two live probes and a three-call load probe;
- an unadmitted row without an explicit blocker;
- duplicate Provider/capability identities;
- evidence paths outside `docs/integrations/`.

This check reads source and documentation only. It does not add Provider
dependencies to the Router and does not execute network probes during ordinary
CI.

## 4. Fixed-offset Timestamp Utilities

`magic-market-core` will expose checked utilities for:

- converting Unix seconds into a canonical RFC3339 timestamp with a validated
  fixed offset;
- the named China Standard Time `+08:00` specialization;
- parsing and comparing a strict `HH:MM:SS` clock.

The conversion uses checked integer arithmetic, Gregorian calendar bounds, and
the same timestamp grammar already accepted by `EvidenceTimestamp`.

Eastmoney post-close/rankings and every current Provider with an equivalent
hand-written `civil_from_days` implementation will migrate to the shared
utility. Provider errors continue to preserve their own typed error enum by
mapping `CoreError`; no timestamp failure becomes an empty batch.

BR-019 comparisons use the strict clock value rather than raw string ordering.
China remains a fixed `+08:00` source contract; this change is centralization,
not a claim that the old epoch conversion depended on the host timezone.

## 5. Numeric Tolerance Policies

`magic-market-core` will add a checked `NumericTolerance` value with explicit
absolute and relative components. Construction rejects negative or non-finite
tolerances. Matching uses:

`abs(left - right) <= absolute + relative * max(abs(left), abs(right))`

Call sites retain named business policies:

- money reconciliation: one cent absolute tolerance;
- order-book totals: relative floating summation tolerance;
- change percentage: percentage-point tolerance;
- source decimal fields: tolerance derived from source precision;
- trade amount: its existing absolute-plus-relative source contract.

Exact `PartialEq` remains available for identity, copied source values, and
serialization round trips. No universal fixed epsilon is introduced.

## 6. Release-profile Evidence

A deterministic release benchmark will exercise representative hot paths:

- TDX variable decoding and bar parsing;
- JSON decoding and normalization;
- a bounded compression/decompression path when an existing fixture supports
  it.

The benchmark consumes tracked fixtures, emits a checksum, iteration count,
elapsed time, throughput, and binary size, and performs no network I/O.

The default profile is compared with `lto="thin"` and
`codegen-units=1` over at least five alternating runs after one warm-up.
The optimized profile is committed only if:

- median throughput improves by at least five percent on the combined hot-path
  workload;
- every checksum is identical;
- no individual workload regresses by more than five percent;
- packaged binary size grows by no more than twenty percent.

Otherwise the default profile remains and the measured non-change is recorded
in `docs/PERFORMANCE_RESULTS.md`. No unmeasured 10-30% claim is added.

## 7. Verification and Release

Focused test-first changes are followed by the repository release gates:

1. formatting;
2. workspace all-target/all-feature check and tests;
3. strict Clippy;
4. Rustdoc and doctests;
5. documentation links;
6. compliance including the new admission registry;
7. production and critical-path coverage thresholds;
8. package verification;
9. release preflight;
10. final diff and provenance review.

Live probes are not rerun merely to make tests green. Existing admitted rows
retain their recorded evidence unless the code changes their source protocol;
any such protocol change requires a fresh bounded live/load admission run.

## Failure and Rollback Semantics

- TDX callers receive typed parse errors where they previously received
  partial or zero-valued records.
- Exchange transport failures remain typed and retain the selected TLS backend
  in operator evidence.
- Admission drift fails compliance before packaging.
- Timestamp and tolerance construction errors propagate through existing
  Provider/Core error mappings.
- Each subsystem is committed separately so it can be reverted without
  removing unrelated hardening.
