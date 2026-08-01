# Eastmoney Provider Top-N Settled-Date Capture Design

**Date:** 2026-08-01
**Status:** Gate A approved for implementation
**Rules:** BR-009, BR-010, BR-011, BR-021, BR-033, BR-034

## 1. Problem

The admitted Provider Top-N endpoint is a current snapshot endpoint rather than
a historical endpoint. On a closed China calendar day it still returns the
latest settled trading session in every selected row's source field `f297`.
The existing contract rejects the request before network access unless the
requested trading date equals the capture calendar date. Consequently a
Saturday post-close consumer cannot validate Friday's settled snapshot even
though the response itself identifies Friday exactly.

Treating the Saturday capture date as the trading date is incorrect. Treating
the endpoint as arbitrary historical retrieval is also incorrect.

## 2. Decision

Separate the requested trading date from the later observation date while
retaining exact source-date validation:

- a request date later than the current Asia/Shanghai date is rejected before
  transport;
- when capture occurs on the requested date, both acquisition start and
  post-response observation remain restricted to `15:35:00` or later;
- when capture occurs after the requested date, acquisition may occur at any
  time of day;
- capture before the requested date, a missing/invalid `+08:00` observation,
  or a response that crosses capture-calendar midnight is rejected;
- every returned row's `f297` must still equal the requested trading date;
- the response remains one provider-ordered page with no full-market coverage
  claim and no fabricated `source_at`.

This admits only the provider's latest settled session. An arbitrary older
request reaches the source but fails atomically when `f297` does not match. A
downstream trading calendar remains responsible for deciding whether that
settled session satisfies its daily-data freshness policy.

## 3. Failure modes

| Failure | Result |
| --- | --- |
| request date is in the future | invalid request before network |
| same-date capture starts or completes before 15:35 | invalid request |
| capture timestamp predates request date | invalid request |
| acquisition and completion cross capture-date midnight | invalid request |
| any selected `f297` differs from request date | whole batch protocol failure |
| transport/schema/order/cardinality/evidence failure | whole batch failure |

No cache, inferred holiday, calendar formula, fallback date, partial batch or
locally fabricated provider time is introduced.

## 4. Old modules

| module | decision | reason |
| --- | --- | --- |
| `ProviderTopNRankingRequest` | adopt | already binds the exact desired trading date |
| Eastmoney `f297` row validation | adopt unchanged | authoritative settled-session evidence |
| same-calendar-date equality gate | replace | conflates observation date with trading date |
| complete-universe ranking route | reject | cannot prove complete-market coverage |

## 5. Validation

```bash
cargo fmt --all -- --check
cargo test -p magic-eastmoney-rs provider_top_n_rankings --locked
cargo test -p magic-market-composition --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
bash tools/compliance/check.sh
bash tools/docs/check_links.sh
```

Deterministic tests must prove same-day pre-window rejection, same-day
post-window admission, later-calendar-date admission, earlier-calendar-date
rejection, future-request pre-network rejection, and exact `f297` binding.

## 6. Rollback

Revert this isolated change. Consumers then return to explicit closed-day
unavailability; no alternative source or fabricated data path is enabled.
