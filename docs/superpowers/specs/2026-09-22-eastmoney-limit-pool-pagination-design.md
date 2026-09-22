# Eastmoney limit-pool pagination design

## Gate A boundary

This is decision 5 of
[`2026-09-21-limit-pool-route-completeness-design.md`](2026-09-21-limit-pool-route-completeness-design.md),
which registered this work as its own Gate A:

> Making Eastmoney paginate would let it return complete batches and stay a
> meaningful first candidate, but it changes the HTTP request pattern on a host
> carried as a provider-local reviewed exception, so it requires its own Gate A
> design and a matching `http-transports.tsv` update.

It changes one adapter (`crates/magic-eastmoney-rs/src/limit_pool.rs`) and amends
BR-028. It adds **no host and no path**: `push2ex.eastmoney.com` is already on
the allowed-host list in
[`eastmoney-web.md`](../../integrations/eastmoney-web.md) and the four endpoint
paths are unchanged. The only wire change is that `Pageindex` advances instead
of being pinned to `0`, so a family whose pool exceeds one page is read in
several requests instead of one. The `magic-eastmoney-rs` row in
[`http-transports.tsv`](../../integrations/http-transports.tsv) changes only its
`reason` text; its `mode`, `direct_dependencies`, `shared_transport` and
`migration_status` are computed from `Cargo.toml` and are untouched.

## Problem

Two independent defects, found while reproducing the `UpperLimitPoolReview`
failure left open by the 2026-09-21 design.

### 1. The adapter asks for one page and calls the result the pool

`crates/magic-eastmoney-rs/src/limit_pool.rs:35-36` sends
`Pageindex=0&pagesize=<caller limit>` and never asks for a second page, then
compares the source's declared total (`:135-144`):

```rust
let issues = (source_total > records.len())
    .then(|| format!(
        "limit-pool caller page is incomplete: source_total={source_total} returned_rows={}",
        records.len()
    ))
    .into_iter()
    .collect();
context.finish_with_issues(records, issues)
```

BR-028 clause 5 sanctions that truncated best-effort batch, so the adapter is
conformant — but it means Eastmoney can never return a complete pool when the
caller's `limit` is below the pool size. The 2026-09-21 route fix routes around
this for `LimitPools`; it cannot for `UpperLimitPoolReview`, whose Eastmoney
handler calls `client.limit_pool` directly, has no other candidate, and needs
`PreviousUpper`, which no other registered Provider serves
(`grpc_production.rs:3498-3532`).

### 2. `qdate` does not mean what the adapter assumes

BR-028 clause 3 and `parse_qdate` require `data.qdate` to equal the **requested**
trading date. Live probing shows `qdate` is the **server's current trading
date**, not the pool's date: it was `20260922` for every date probed, including
dates whose row counts differed correctly. So the equality can only hold when
the caller asks for the server's current trading date, and every historical
request fails the guard.

That failure is currently correct behaviour, for a reason worth recording: the
rows carry no date field (`c`, `m`, `n`, `p`, `zdp`, `lbc`, `fbt`, `lbt`,
`fund`, `zbc`, `hybk`), so `qdate` is the *only* evidence binding the payload to
a date. Dropping the guard would publish rows whose date nothing can prove. The
defect is the diagnosis, not the guard: the error reads as a data fault when it
is a statement about what this endpoint can prove.

## Observed evidence

Direct HTTPS `GET` to `https://push2ex.eastmoney.com` on 2026-09-22 between
08:53 and 09:05, 20 requests at least 1.2 s apart, no credentials — the `ut`
query value is the adapter's own public constant. `data` has exactly three keys,
`pool`, `qdate` and `tc`.

### `Pageindex` is a real, 0-based page address, and `tc` is stable across pages

`getTopicZTPool`, `sort=fbt:asc`:

| `Pageindex` | `pagesize` | `tc` | `rows` | first code |
| --- | --- | --- | --- | --- |
| 0 | 5 | 103 | 5 | `001216` |
| 1 | 5 | 103 | 5 | `603230` |
| 2 | 5 | 103 | 5 | `601123` |
| 0 | 200 | 103 | 103 | `001216` |
| 0 | 1000 | 103 | 103 | `001216` |

The three size-5 pages are disjoint, so `Pageindex` selects a page rather than
repeating one, and `tc` does not drift while pages are read.

### `date` selects the data; `qdate` reports the server's current date

`getTopicZTPool`, `pagesize=3` unless noted:

| requested `date` | `tc` | `rows` | `qdate` |
| --- | --- | --- | --- |
| 20260916 | 89 | 3 | **20260922** |
| 20260917 | 47 | 3 | **20260922** |
| 20260918 | 78 | 3 | **20260922** |
| 20260921 | 103 | 3 / 5 / 103 | **20260922** |
| 20260922 | 0 | 0 | **20260922** |

`tc` tracks the requested date exactly, so `date` filters the result. `qdate`
does not: it is the server's current trading date in every case.

### The other three families behave the same way

| family | path | `sort` | `date` | `tc` | page 0 / page 1 (`pagesize=5`) |
| --- | --- | --- | --- | --- | --- |
| `PreviousUpper` | `getYesterdayZTPool` | `zs:desc` | 20260922 | 102 | 5 rows / 5 rows, disjoint (`000020` / `000566`) |
| `Broken` | `getTopicZBPool` | `fbt:asc` | 20260922 | 0 | verified-empty pre-open |
| `Lower` | `getTopicDTPool` | `fund:asc` | 20260922 | 0 | verified-empty pre-open |

`getYesterdayZTPool` with `pagesize=1000` returned all 102 rows, and `qdate` was
`20260922` there too.

Two things this evidence does **not** establish, and which this design therefore
does not assert:

- **The server's `pagesize` ceiling.** `pagesize=200` and `pagesize=1000` both
  returned the whole 103-row pool, which proves no ceiling below 103 and nothing
  more. Every pool probed today is smaller than any plausible ceiling.
- **Whether `getTopicZBPool` and `getTopicDTPool` paginate.** Both returned
  `tc=0` pre-open, so their paging was not exercised. They share the envelope
  and path family but are not observed, and Gate C must re-check them on a day
  with non-empty pools.

### Two families disagree about the same trading date

`getTopicZTPool date=20260921` reports `tc=103`, while `getYesterdayZTPool
date=20260922` — the previous-upper pool for the same 2026-09-21 session —
reports `tc=102`. The families are different sources and no rule may assert that
their totals agree. `UpperLimitPoolReview` already reads them independently and
`validate_pool` (`derived_products.rs:1147-1176`) does not compare families
across kinds; that must stay true.

## Decision

### 1. The adapter fetches until it has the declared total (amends BR-028)

`LimitPools for EastmoneyClient` replaces its single pinned request with a
bounded page walk:

```rust
const LIMIT_POOL_PAGE_SIZE: u32 = 200;
/// 5 * 200 = 1000, the registered limit-pool row ceiling.
const MAX_LIMIT_POOL_PAGES: u32 = 5;
```

- Walk `Pageindex` from `0` while `Pageindex < MAX_LIMIT_POOL_PAGES`, holding
  `pagesize` at `LIMIT_POOL_PAGE_SIZE` and keeping `ut`, `dpt`, `sort` and `date`
  identical to today's request.
- Page 0 establishes the contract: its `tc` and its parsed `qdate`.
- Every later page must report the same `tc` and the same `qdate`. A difference
  means the source changed while it was being read, and is a protocol failure
  rather than a shorter batch.
- Stop as soon as the accumulated row count equals `tc`.
- A page returning zero rows while fewer than `tc` have been accumulated is a
  protocol failure.
- Reaching the page bound with fewer than `tc` rows is a protocol failure naming
  both counts — never a truncated batch. This mirrors the HithinkFinance handler,
  which errors with "limit pool requires N pages, exceeding bound
  `MAX_LIMIT_POOL_PAGES`" rather than truncating.

The walk deliberately does **not** assert that a page returned
`LIMIT_POOL_PAGE_SIZE` rows. The ceiling is unobserved, and if the server clamps
`pagesize` the `collected == tc` loop is unaffected while a cardinality
assertion would fail on a healthy response. This is the one place the design
departs from the HithinkFinance pattern, which can assert per-page cardinality
because its envelope echoes `size` back (`magic-hithink-rs/src/lib.rs:1074-1090`);
Eastmoney's does not.

After the walk: deduplicate instrument identity across the whole set (the
existing check, now spanning pages), truncate locally to `request.limit()`, and
return `context.finish(records)` — a strict, complete batch.

**Truncating locally after proving the total is what keeps the caller's bound
honest.** The caller still receives at most `limit` rows; what changes is that
`complete` now means the pool behind those rows was read whole. This is exactly
what the HithinkFinance handler already does (`records.truncate(request.limit()
as usize)` after proving `total`), so the two Providers converge on one
semantics rather than diverging.

BR-028 clause 5's truncated best-effort batch becomes unreachable for this
adapter: `tc > rows.len()` can no longer be returned, because the adapter either
reaches `tc` or fails. Clause 6 is unchanged and now always satisfiable. The
issue mechanism itself stays in `BatchContext` and is still reached by
`finish` and `finish_allow_empty`.

### 2. The `qdate` guard stays; only its diagnosis changes

The equality check remains, because the evidence above shows it is the only
thing that can bind the payload to a date, and the correct behaviour for an
unprovable date is to fail closed.

What changes is the message. Today it reads as a data fault ("source qdate
2026-09-22 does not match requested date 2026-09-21"). It should say what is
actually true: this endpoint reports its own current trading date, so it can
prove a batch only for that date. The error variant does not change, so neither
the route's behaviour nor any admission state moves.

The consequence is recorded plainly rather than worked around: **Eastmoney
limit-pool results are verifiable only for the server's current trading date.**
Same-day review of the current session's pool — which is the `UpperLimitPoolReview`
use the downstream monitor makes — is unaffected. Reviewing a past date is not
served by this Provider, and decision 1 does not change that.

### 3. The transport registry records the request pattern, not a new host

The `magic-eastmoney-rs` row's checked fields are derived from `Cargo.toml` and
do not change, so `tools/compliance/check_http_transports.py` passes either way.
The `reason` text is updated to record that the limit-pool family is now
multi-request on an already-allowlisted host and path — the update the 2026-09-21
design promised. No `*_ADMITTED` constant is introduced and no `admissions.tsv`
row is added: the limit-pool family is not admission-registered today (the only
`LIMIT_POOLS_ADMITTED` row belongs to HithinkFinance), this design does not
change that, and adding a row would additionally require the 2 live / 3 serial
probe counts that `check_admissions.py` enforces.

## Cost

`eastmoney-web.md` fixes one request in flight per clone with request starts at
least 1 s apart, so pages are serial and each costs at least a second.

Pages per family is `ceil(tc / LIMIT_POOL_PAGE_SIZE)`. For any pool of 200 rows
or fewer this is **1**, so the request count is unchanged — including every pool
observed today (Upper 103, PreviousUpper 102) and `UpperLimitPoolReview`'s four
families, which stay at four requests. The count only grows once a family passes
200 rows, and is bounded at the 1000-row contract ceiling by 5 pages × 4
families = 20 requests, at least 20 s, inside the 12 s per-request timeout and
well inside the caller's budget.

## Open question (not decided here)

`parse_qdate` reports a past-date request as `EastmoneyError::Protocol`, which is
non-retryable and stops the route. Reclassifying it as `Unsupported` would,
under the 2026-09-21 amendment to BR-059, let the route advance to
HithinkFinance — which may well serve a past trading date, since it queries by
`date_ms` and validates its own pagination contract rather than `qdate`. That
would make past-date `LimitPools` work for `Upper`, `Lower` and `Broken`.

It is not decided here because it rests on an unverified premise: **nobody has
established whether the HithinkFinance Fuyao endpoint returns a non-empty,
complete batch for a past trading date.** That needs its own evidence round, and
its own Gate A, before the error classification moves. `PreviousUpper` would not
benefit either way, because no other registered Provider serves it.

## Rejected alternatives

- **Ask for one page of 1000 and rely on it.** Does not remove the defect. The
  server's ceiling is unobserved (see above), and if it clamps the identical
  truncation returns at the identical place. For pools of 200 rows or fewer
  pagination costs exactly the same one request.
- **Keep the truncated batch and let the route fall through.** That is the
  status quo the 2026-09-21 design left in place. It cannot serve
  `UpperLimitPoolReview`, which has no other candidate, and it leaves the first
  registered `LimitPools` candidate permanently unable to satisfy BR-028
  clause 6.
- **Truncate to `request.limit()` without proving `tc`.** Contradicts BR-028 and
  BR-056's "every declared page is fetched and checked [...] before a caller
  limit is applied", and would publish a bounded page as the pool.
- **Assert that each page returns exactly `LIMIT_POOL_PAGE_SIZE` rows.** Invents
  a contract that was not observed, and turns a healthy clamped response into a
  protocol failure.
- **Drop the `qdate` equality guard so past dates resolve.** The payload carries
  no other date evidence, so the rows' date would be unprovable. Rejected on
  Gate B provenance grounds.
- **Propagate the previous page's sort order to the other families.** The sorts
  already differ per kind and are correct (`fbt:asc`, `fbt:asc`, `fund:asc`,
  `zs:desc`); a wrong sort is what produced a spurious `tc=102 rows=0` reading
  during evidence collection, not an endpoint fault.

## Public contract

- Operation/RPC: `LimitPools`, `UpperLimitPoolReview` — names, request schemas
  and record schemas unchanged.
- Providers: `Eastmoney`, `Tonghuashun`, `HithinkFinance` — registration order
  and admission states unchanged.
- Transport: no new host, no new path, no timeout, body-size, redirect, proxy or
  authentication policy change. `push2ex.eastmoney.com` stays `legacy-direct` /
  `reviewed-exception`.
- Changed behaviour: `LimitPools` from Eastmoney returns a batch whose
  `complete` flag means the pool was read whole and whose row count is still at
  most the caller's `limit`. `UpperLimitPoolReview` no longer requires
  `per_pool_limit` to be at or above the pool size for the current trading date.

## Verification

- Deterministic tests in `magic-eastmoney-rs` over a fake transport: pages
  assemble to `tc`; a later page with a different `tc` fails; a later page with a
  different `qdate` fails; a zero-row page before `tc` is reached fails; a pool
  that needs more than `MAX_LIMIT_POOL_PAGES` fails naming both counts; a
  `limit` below `tc` yields `complete=true` truncated to `limit`; the existing
  verified-empty, duplicate-identity, `rc` and `qdate` cases are unchanged.
- `crates/magic-eastmoney-rs/tests/internal/limit_pool_tests.rs:167` currently
  asserts the single-page truncation; it must be rewritten to the new contract.
- Live, against the deployed binary on the current trading date:
  - `LimitPools` `Upper limit=50` → `complete=true`, 50 rows (was
    `complete=false`).
  - `UpperLimitPoolReview {per_pool_limit: 50}` → succeeds (was
    `FailedPrecondition: source_total=103 returned_rows=50`).
  - `UpperLimitPoolReview {per_pool_limit: 200}` → succeeds, unchanged.
  - `UpperLimitPoolReview` for a **past** trading date still fails closed, with
    the corrected message.
- Request-count evidence: for a date whose pool is 200 rows or fewer, the
  limit-pool request count per call is unchanged at one per family.
- Gate C: `cargo fmt --all -- --check`, `cargo test --workspace --lib --bins
  --tests`, `cargo clippy -p magic-eastmoney-rs --all-targets -- -D warnings`,
  `tools/compliance/check.sh` and the documentation link check. As recorded in
  the 2026-09-21 design, `cargo test --workspace --all-targets` cannot link on
  Windows because several crates ship an example named `live_probe` writing to
  one output path.
- Re-check `getTopicZBPool` and `getTopicDTPool` pagination in-session on a day
  with non-empty pools; today's pre-open probes exercised only their
  verified-empty path.

## Rollback

Revert the adapter commit. Eastmoney returns to a single pinned page and the
truncated best-effort batch, `UpperLimitPoolReview` again requires
`per_pool_limit` at or above the pool size for the date, and the 2026-09-21
route fix continues to route `LimitPools` around it. No registry, admission,
transport or Protobuf state is touched, so the rollback is confined to one
adapter and its tests.
