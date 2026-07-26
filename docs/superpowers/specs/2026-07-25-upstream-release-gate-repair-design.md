# Unified Upstream Release-Gate Repair Design

**Date:** 2026-07-25
**Status:** Approved under the standing unified-data migration instruction
**Rules:** BR-020, BR-024

## Objective

Repair only the deterministic defects exposed by the real GitHub release
gates, while preserving explicit transport and source failures. This slice
does not turn an external 302, TLS error, SSE 403, or CFFEX unreachable result
into success.

## Eastmoney limit-pool probe completeness

`LimitPools::limit_pool` intentionally marks a response incomplete when the
source `data.tc` exceeds the returned page. The live probe currently requests
three rows, so a normal 11/25/116-row market day fails admission by design.
The production contract remains unchanged. The live probe requests the
contract maximum of 1,000 rows for each pool. If `data.tc` still exceeds the
returned rows or 1,000-row bound, the probe continues to fail explicitly.

## Dragon-tiger seat identity

Eastmoney may legitimately repeat the display label `机构专用` at several
positions on one side. A seat's normalized business identity is entry ID,
side, and source-order rank. Seat display name and amounts are facts, not
identity fields. Exactly five rows on each side are still required, and their
source order maps to ranks 1 through 5. Missing/extra rows, wrong entry/date/
instrument, invalid side amounts, or duplicate normalized side/rank remain
atomic failures. No row is collapsed because its name or amounts match.

## Eastmoney rolling-news canonical hosts

The official `roll.eastmoney.com/finance.html` source currently emits article
links on both `finance.eastmoney.com` and `global.eastmoney.com`. Both use the
same exact `/a/<numeric-id>.html` contract. The Provider admits those two exact
lowercase hosts only, normalizes the scheme to HTTPS, preserves the source
host/path, and still rejects credentials, ports, query/fragment suffixes,
lookalikes, other Eastmoney properties, and noncanonical paths.

## Redirect diagnostics

The shared Eastmoney transport retains `redirects(0)`. For non-2xx ureq status
errors it records the numeric status and a bounded public `Location` header,
when present. It does not follow the redirect. Board-flow and popularity 302
failures therefore remain failures until an exact endpoint or request contract
is proved by a later real workflow run.

## Validation

- RED/GREEN tests for repeated institutional seat names and the
  `global.eastmoney.com` canonical path;
- live-probe request-shape test for complete limit-pool capacity;
- transport test proving 302 remains an error and exposes the location;
- Eastmoney tests and strict Clippy;
- workspace `--all-targets`, compliance, cargo-deny CI, and real-data workflow.

## Rollback

Revert this slice. If new source evidence contradicts the host or identity
contract, disable that capability and return an explicit error. Do not restore
the three-row completeness probe, name-based seat deduplication, redirect
following, or fabricated records.
