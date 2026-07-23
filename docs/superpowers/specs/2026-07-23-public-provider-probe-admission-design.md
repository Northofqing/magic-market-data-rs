# Public Provider Probe Admission Design

**Date:** 2026-07-23  
**Status:** Approved Task 8 Gate A supplement

Parent designs:

- `docs/superpowers/specs/2026-07-23-a-stock-data-parity-design.md`
- `docs/superpowers/plans/2026-07-23-public-intelligence-providers.md`
- `docs/superpowers/plans/2026-07-23-magic-market-data-slice-0-baseline.md`

## Problem

The current probes often treat `Ok(DataBatch)` as success. That is insufficient:
`DataBatch::strict` may be empty, quality may be incomplete, record evidence
may disagree with batch provenance, timestamps may be invalid, and a diagnostic
or skipped credential path may look like an admitted capability.

## Admission state model

Every family ends in exactly one stable machine state:

| State | Meaning |
| --- | --- |
| `admitted` | Advertised family; all data, quality, provenance, identity, freshness, unit, and cross-field gates pass |
| `verified_empty` | Advertised family; the source explicitly proves a legitimate empty result with request identity and provenance |
| `diagnostic_complete_unadmitted` | Real diagnostic completed, but the family is false or a required invariant remains unproved |
| `skipped_missing_secret` | Credentialed diagnostic was not attempted because its secret is absent |
| `failed` | Transport, authentication, schema, quality, provenance, freshness, identity, or pacing gate failed |

Only `admitted` and `verified_empty` satisfy a declared capability. A probe
must not emit `passed` for unadvertised, incomplete, skipped, or diagnostic
families.

## Shared verifier

`magic-market-core` owns an additive provider-neutral verifier; existing
production Provider traits are not renamed. Callers supply record evidence and
business-identity accessors because Core records do not share one identity
field.

An admitted batch requires:

1. at least one record;
2. complete quality and no issues;
3. non-empty batch source and batch identity;
4. record provider, observed time, source time, and batch identity equal to
   batch provenance;
5. no source timestamp later than observation;
6. caller-selected freshness where source time is supplied;
7. unique, non-empty business identities.

`VerifiedEmpty` carries family, request identity, provider, observed time,
optional source time, batch identity, and a non-empty source reason. It is
created only when the wire response explicitly proves no records. Transport
errors, missing fields, login pages, and schema drift remain failures.

## First implementation slice

Tonghuashun “no institution estimate” returns typed `VerifiedEmpty` evidence
and no `ConsensusSnapshot` pseudo-record. Incomplete estimates cannot satisfy
the live admission verifier. Its live probe prints a stable `verified_empty`
state only for that typed outcome.

Tests for touched private parser seams remain path-based unit submodules. This
preserves private access without exposing testing-only public APIs or counting
test bodies as production-file coverage.

## Follow-up slices

- Eastmoney applies BR-011 to Dragon-Tiger and other principal atomic families
  and validates non-negative gross values plus amount/net arithmetic.
- iWencai separates unadmitted diagnostic success, missing-secret skip, and
  failure in code, CI, and documentation.
- Baidu remains diagnostic/unadmitted until latest-session, trading-calendar,
  adjacent-change, and corporate-action continuity are proved.
- Every advertised family gets a bounded load case that measures actual
  request-start spacing and observed concurrency.

## Failure handling and rollback

Ordinary empty batches, incomplete quality, issues, provenance mismatch,
future/stale source time, duplicate identities, and inconsistent units or
cross-fields fail explicitly. Unknown facts are not inferred.

Changes are split by design, shared verifier/THS, Eastmoney, iWencai, Baidu,
load evidence, and documentation. Each slice can be reverted independently
with `git revert <sha>`. Real network probes are run separately and no offline
test is described as live evidence.

