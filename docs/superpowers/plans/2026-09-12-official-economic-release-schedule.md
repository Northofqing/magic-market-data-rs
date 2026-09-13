# Official Economic Release Schedule Implementation Plan

**Goal:** Add a truthful, date-only FRED release-schedule operation without
weakening the existing economic-calendar contract.

**Seam:** `EconomicReleaseScheduleProvider` in Core is the public interface;
FRED owns HTTP and parsing implementation; gRPC owns schema projection.

## Vertical slice 1: Core contract

- Add checked request, record, Provider interface, evidence implementation and
  invariant tests in `magic-market-core`.
- Keep date-only schedule facts distinct from release observations.

## Vertical slice 2: FRED adapter

- Add deterministic red tests for exact request construction, pagination,
  date filtering, zero records, duplicate identity and incomplete pages.
- Extend only the existing FRED endpoint policy and query-key allowlist.
- Implement typed parsing and atomic acquisition behind the Core seam.

## Vertical slice 3: gRPC composition

- Append one operation/RPC and v1 request/record schemas.
- Register FRED as diagnostic until live evidence passes.
- Add exact registry, capability, serialization and typed-failure tests.

## Vertical slice 4: admission and release

- Run two credentialed live probes and a three-call serial load probe.
- If and only if all evidence passes, set the FRED admission constant, registry
  row and production capability to admitted.
- Update integration docs, external gRPC docs, bundle counts and checksums.
- Run formatting, focused/workspace tests, Clippy, compliance, documentation
  links and release preflight before commit or deployment.
