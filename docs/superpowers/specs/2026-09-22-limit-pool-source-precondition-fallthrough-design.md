# Limit-pool source-precondition fall-through design

## Gate A boundary

This is the evidence round that
[`2026-09-22-eastmoney-limit-pool-pagination-design.md`](2026-09-22-eastmoney-limit-pool-pagination-design.md)
registered as its own Gate A. That design's closing open question:

> `parse_qdate` reports a past-date request as `EastmoneyError::Protocol`, which is
> non-retryable and stops the route. Reclassifying it as `Unsupported` would, under
> the 2026-09-21 amendment to BR-059, let the route advance to HithinkFinance [...]
> It is not decided here because it rests on an unverified premise: **nobody has
> established whether the HithinkFinance Fuyao endpoint returns a non-empty,
> complete batch for a past trading date.** That needs its own evidence round, and
> its own Gate A, before the error classification moves.

The evidence round is in *Evidence* below. It establishes the premise, and it
establishes something the open question did not assume: **`Tonghuashun` also serves
a past trading date**, so the `Upper` kind gains a second candidate, not just
`HithinkFinance`.

It changes one function (`OperationRegistry::execute_limit_pool_route` in
`crates/magic-market-service/src/lib.rs`) and amends BR-059. It adds no Provider, no
host, no path, no transport, no admission state and no Protobuf field. No
`http-transports.tsv` or `admissions.tsv` row changes.

The error classification does **not** move: this design does not reclassify the
`qdate` divergence as `Unsupported`, and does not touch `parse_qdate`. What moves is
the *route's* reading of the error it already receives.

## Problem

`execute_limit_pool_route` stops the route on any non-retryable, non-scope-declined
failure:

```rust
let scope_declined = matches!(error, ServiceError::Unsupported { .. });
if !retryable && !scope_declined {
    return Err(ServiceError::ProviderRouteFailure { exhausted: false, attempts });
}
```

`ServiceError::FailedPrecondition` is one of those failures
(`provider_attempt_from_error` records it as `rejected`/`source_precondition`,
non-retryable). Eastmoney is the **first** registered `LimitPools` candidate:
`register_eastmoney` is called at `grpc_production.rs:705`,
`register_additional_providers` (Tonghuashun) at `:802` and `register_hithink` at
`:1300`, so registration order is Eastmoney → Tonghuashun → HithinkFinance.

Eastmoney's limit-pool adapter requires `data.qdate` to equal the requested trading
date (`crates/magic-eastmoney-rs/src/limit_pool.rs:147-166`). `qdate` is the source's
**current** trading date. So whenever the requested date is not the source's current
trading date, the first candidate returns a non-retryable precondition and the route
stops before Tonghuashun or HithinkFinance is ever asked.

Two live consequences:

### 1. The recurring pre-open outage

On both 2026-09-20 and 2026-09-22 the downstream monitor's 09:15–09:25 call-auction
window failed, and self-healed at 09:30. The server's own record for 2026-09-22 is
35 route-failure lines between 09:13 and 09:24 local, every one of them:

```
level=ERROR target=grpc_server event=provider_route_failure
stage=provider_route_stopped attempt_count=1 attempts=Eastmoney:source_precondition
```

The capture those 35 lines are counted from is committed whole, with all 35
records and the pinned per-candidate calls taken in the same window, as
[`docs/evidence/2026-09-22-limit-pools-window-capture.md`](../../evidence/2026-09-22-limit-pools-window-capture.md).

That artifact also dates the recovery more tightly than the monitor's report: the
server's last stop is 09:24:44 and its first success is 09:25:04, so the boundary
is 09:25 local rather than 09:30. The two are not reconciled here — a monitor
reports the samples it took, not the server's first success.

During the window Eastmoney's `qdate` is still the previous trading date, so the
guard fires, the route stops at candidate 1, and the two candidates that could have
answered are never called. After the open `qdate` becomes the requested date and the
route self-heals.

The same guard fires for a past-date request, which is why a past trading date is a
deterministic stand-in for a window that only recurs pre-open.

### 2. A latent landmine for the other kinds

Independently of the clock, a past-date `LimitPools` request for `Upper`, `Broken` or
`Lower` fails today even though a registered candidate can serve it (see *Evidence*).
`UpperLimitPoolReview` has the same shape.

## Evidence

Live, against the running production registry (2026-09-22 21:26 local), `limit=10`:

Requested `2026-09-21`, a past trading date:

| kind | Eastmoney | Tonghuashun | HithinkFinance | unpinned route (as shipped) |
| --- | --- | --- | --- | --- |
| `Upper` | `FailedPrecondition` qdate mismatch | **complete · 10 rows · source_at=2026-09-21** | **complete · 10 rows · source_at=2026-09-21** | **fail** `provider_route_stopped` · attempt_count=1 · `Eastmoney:source_precondition` |
| `Broken` | `FailedPrecondition` qdate mismatch | `Unimplemented` (scope decline) | **complete · 10 rows** | **fail** `provider_route_stopped` · attempt_count=1 |
| `Lower` | `FailedPrecondition` qdate mismatch | `Unimplemented` (scope decline) | **complete · 2 rows** | **fail** `provider_route_stopped` · attempt_count=1 |

Control, requested `2026-09-22` (the source's current date): all three candidates
return complete batches, and the unpinned route succeeds. So the failing ingredient
is the date divergence alone, not the request, the callers, or Provider health.

The Eastmoney error verbatim:

```
Code: FailedPrecondition
Message: Eastmoney protocol error: limit-pool source qdate 2026-09-22 does not
         match requested date 2026-09-21
```

That message is a statement about **the source's own published date**. It is not a
statement about the request: the request is well formed, the instruments are
irrelevant to the guard, and two admitted candidates return complete, correctly
dated, correctly provenanced batches for the same request.

BR-059's own rationale for advancing already covers it:

> Two outcomes advance to the next candidate instead of terminating, **because each
> is a fact about that candidate rather than about the request**

A source that has not published the requested date is precisely a fact about that
candidate.

## Decision

### 1. A candidate-scoped source precondition advances the route (amends BR-059)

`execute_limit_pool_route` gains a third advance condition. A
`ServiceError::FailedPrecondition` from a candidate:

- is recorded as a bounded `rejected`/`source_precondition` attempt, the closed
  vocabulary `provider_attempt_from_error` already produces — no new reason code and
  no free-form upstream text enters `ProviderAttempt`;
- does **not** terminate the route;
- does **not** count as a scope decline.

It is deliberately not counted with the scope declines. A scope decline means "no
registered capability serves this", and BR-059 says a scope that every candidate
declines stays that unchanged unsupported-scope error, which is a useful and
specific answer. A precondition decline means "the candidates that do serve this
could not attest this date", which is an ordinary exhausted route over candidates
that serve the operation. So an all-precondition-declined request returns
`ProviderRouteFailure { exhausted: true, attempts }` and keeps the bounded attempt
trace of every candidate tried, rather than collapsing to a bare error.

The route's fail-closed guarantee is unchanged. A candidate still speaks only with a
**complete** batch, a candidate that cannot prove the pool is whole still advances,
an explicit `preferred_provider` still never falls through, and an exhausted or
stopped route still returns only bounded typed attempts — never stale records, mixed
provenance or partial success.

### 2. What stops the route is unchanged

`invalid_request`, `unauthenticated`, `permission_denied`, `invalid_evidence`,
scope-exhaustion and every transport or protocol fault keep stopping or failing
exactly as they do today. This design moves one error variant, not the policy.

### 3. The date guard itself is untouched

`parse_qdate` keeps requiring `qdate` to equal the requested date. Dropping the
equality would let Eastmoney answer a past date with a batch whose date it cannot
prove, which was rejected on Gate B provenance grounds in the pagination design and
stays rejected. Eastmoney remains unverifiable for any date but its current one;
this design only stops that limitation from speaking for the whole route.

## What the client sees

| case | before | after |
| --- | --- | --- |
| a later candidate can attest the date | `FAILED_PRECONDITION` · `provider_route_stopped` · retryable=false | **`OK`** · that candidate's complete batch |
| no candidate can attest the date | `FAILED_PRECONDITION` · `provider_route_stopped` · retryable=false · attempts trailer | `FAILED_PRECONDITION` · `provider_route_exhausted` · retryable=false · attempts trailer |
| every candidate declines the scope | `UNIMPLEMENTED` · the unchanged unsupported error | unchanged |

One client-visible string changes, and only in the case where the request still
fails: `provider_route_stopped` becomes `provider_route_exhausted`, which is the more
accurate of the two, because the route now demonstrably tried every candidate.
`retryable`, the gRPC code, the reason-code vocabulary, the request schemas, the
record schemas and the `magic-error-detail-bin` trailer shape are unchanged.
[`grpc-external-api.md`](../../integrations/grpc-external-api.md) documents both
strings already, so no contract document needs a vocabulary change.

## Observability

The all-precondition path returns `ProviderRouteFailure`, which the boundary already
records as `event=provider_route_failure` with `request_id`, `operation`,
`attempt_count` and the ordered provider/reason list. So the operator-visible record
for a still-failing request survives this change, and gains the whole attempt list
that the previous single-candidate stop could not produce.

The complementary gap — a client-visible `INTERNAL` that produces **no** record at
all — is fixed separately in
[`2026-09-22-internal-failure-observability-design.md`](2026-09-22-internal-failure-observability-design.md).

## Tests

`crates/magic-market-service/src/lib.rs`:

- `non_retryable_limit_pool_failure_stops_with_safe_attempt` asserts the **old**
  policy (`FailedPrecondition` stops the route, the second candidate is never
  called). It is replaced by
  `source_precondition_limit_pool_falls_through_to_an_attesting_provider`, which
  asserts the second candidate is called and its complete batch is returned.
- `every_source_precondition_limit_pool_reports_an_exhausted_route` — new. Both
  candidates decline by precondition; asserts `exhausted: true`, two ordered
  `source_precondition` attempts and no data.
- `invalid_request_limit_pool_failure_stops_the_route` — new. Pins that a genuinely
  non-retryable request fault (`InvalidRequest`) still stops the route with
  `exhausted: false` and one attempt, so the amendment cannot quietly widen into
  "every non-retryable failure advances".
- `scope_every_limit_pool_provider_declines_stays_unsupported` and
  `scope_declining_limit_pool_falls_through_to_a_serving_provider` must keep passing
  unchanged: no scope-decline semantics move.

## What this does not claim

It does **not** claim the 09:15–09:25 window is restored. That window only exists
pre-open, and this evidence round ran after the close, so the past-date divergence
stands in for a guard that is provably the same code path but is not the same clock.
Whether Tonghuashun or HithinkFinance can attest **today's** pool at 09:15 — before
today's pool exists — is not established here, and a candidate that cannot attest it
will make the route exhaust, which is a correct and unchanged failure.

The falsifiable follow-up, which needs no code change: run the same probe in the
09:15–09:25 window on the next trading day and read `stage=provider_route_exhausted
attempts=...`. If the attempt list now names the later candidates, the artificial
stop is gone; if the window still fails, the remaining cause is candidate readiness
and is not this defect.
