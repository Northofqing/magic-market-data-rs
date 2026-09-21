# Limit-pool route completeness design

## Gate A boundary

This design is written on the caller's 2026-09-21 instruction to reproduce the
limit-up-pool (`LimitPools`) failure reported by the downstream monitor inside
the 09:15-09:25 auction window, and to produce a Gate A design before any
contract change.

It amends BR-059, and adds one field to one log line. It changes no registered
scope string, no Provider adapter, no HTTP host, path, timeout, body-size,
redirect, proxy or authentication policy, no Protobuf message, no request or
record schema, and no admission state. Every request stays blocking and stays in the existing bounded
concurrency lane. `push2ex.eastmoney.com` keeps its current `legacy-direct`
reviewed-exception row in
[`http-transports.tsv`](../../integrations/http-transports.tsv); this design adds
no request to it.

The route change is confined to `execute_limit_pool_route`
(`crates/magic-market-service/src/lib.rs:520-585`), which is service-layer
policy and performs no I/O itself.

## Problem

An unpinned `LimitPools` query failed with `FailedPrecondition / provider route
did not produce an admitted batch` while two of its three registered production
Providers answered the same request completely.

The failure is not window-bound. It reproduces on demand outside the auction
window, and the probe scripts written for the window
(`target/runtime/window-failure-repro.ps1`,
`target/runtime/limit-pools-route-diag.ps1`) are sufficient to trigger it.

BR-059 states the route's contract:

> An unpinned `LimitPools` query may try the registered production Providers in
> their deterministic registration order only after an explicitly retryable
> availability, timeout or rate-limit failure. [...] A successful complete batch,
> including a truthful verified-empty batch, terminates the route immediately.

The route exists so that a caller asking for the pool of one date receives *a*
complete admitted batch. The implementation instead terminates on the **first
candidate** whenever that candidate does not return a complete batch, so the
route is only ever as good as its first registration.

BR-028, the approved design for this same adapter
([`2026-07-25-eastmoney-limit-pool-completeness-design.md`](2026-07-25-eastmoney-limit-pool-completeness-design.md)),
already sanctions the batch that stops it:

> 5. `tc > validated_rows.len()` yields a best-effort batch with an explicit
> truncation issue containing both counts.
>
> 6. Downstream whole-market consumers must require `quality.is_complete()`. A
> bounded consumer may inspect incomplete rows only if its own registered rule
> explicitly allows that behavior; it may never call them whole-market data.

So the incomplete batch is a deliberate, correctly-flagged quality state, not a
fault. The defect is that the route reads it as one.

## Observed evidence

All probes are live production gRPC calls over mTLS with the runtime Bearer
token, against the deployed 2026-09-21 binary. `kind` is the `LimitPoolKind`,
`target` is `route` (no `preferredProvider`) or a pinned Provider.

### The route stops after exactly one attempt

```
=== LimitPools [repro-limitpools-1] exit=73 ===
Code: FailedPrecondition
Message: provider route did not produce an admitted batch
```

The `magic-error-detail-bin` trailer decodes to a single attempt:

```
[Eastmoney][provider_route_stopped][Eastmoney][rejected][response_invalid]
```

`GetCapabilities` gives the registration order and state for
`OPERATION_LIMIT_POOLS`: **(1) Eastmoney, (2) Tonghuashun, (3) HithinkFinance**,
all `ADMISSION_STATE_ADMITTED` with `runtime_available=true`. Eastmoney is
first, and it is the one that stops the route.

### The same providers succeed individually

| kind | route | Eastmoney | Tonghuashun | HithinkFinance |
| --- | --- | --- | --- | --- |
| `Upper` | **fail** `provider_route_stopped` | `complete=`**false** · 50 rows | `complete=true` · 50 rows | `complete=true` · 50 rows |
| `Lower` | ok `complete=true` · 2 rows | `complete=true` · 2 rows | **`Unimplemented`** · upper only | `complete=true` · 2 rows |
| `Broken` | ok `complete=true` · 25 rows | `complete=true` · 25 rows | **`Unimplemented`** · upper only | `complete=true` · 25 rows |

An empty `complete=` is grpcurl omitting a `false` default; Eastmoney returned 50
rows with `complete=false`.

### The trigger is the caller-controlled `limit`

| kind | `limit` | route result |
| --- | --- | --- |
| `Broken` | 25 | ok · `complete=true` · 25 rows |
| `Broken` | **10** | **fail** `provider_route_stopped` |
| `Lower` | 2 | ok · `complete=true` · 2 rows |
| `Lower` | **1** | **fail** `provider_route_stopped` |
| `Upper` | **200** | ok · `complete=true` · **103** rows |

`limit_pool` sends `Pageindex=0&pagesize=<caller limit>`
(`crates/magic-eastmoney-rs/src/limit_pool.rs:35-36`) and never paginates, then
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

The 2026-09-21 `Upper` pool holds 103 members, so `limit=50` yields
`source_total=103 returned_rows=50` and an incomplete batch; `limit=200` covers
the pool and yields a complete one. **The unpinned route therefore succeeds if
and only if the caller's `limit` is at least Eastmoney's pool size for that date
and kind.**

A pool grows through the session as more instruments seal the limit. A `limit`
that covers the pool pre-open stops covering it once the pool fills, so the same
call starts failing partway through the morning — which is the window dependence
the downstream observed. The downstream monitor asks for a bounded top-N, which
is the failing case by construction.

### The derived product hits the same trigger but not this defect

`UpperLimitPoolReview` (`per_pool_limit` is its caller bound, and BR-050 requires
all four exact pool families) fails with the same message shape:

```
{"trading_date":"2026-09-21","per_pool_limit":50}
-> FailedPrecondition: Eastmoney upper-limit review input is incomplete:
   limit-pool caller page is incomplete: source_total=103 returned_rows=50
   detail: [source_precondition_failed]

{"trading_date":"2026-09-21","per_pool_limit":500}
-> ok, admission=ADMITTED, provider=Eastmoney, complete=true, records=1
```

It is **not** fixed by this design, and deliberately so. Its Eastmoney handler
calls `client.limit_pool` directly rather than through the route, so it neither
has other candidates to fall through to nor any candidate that could supply
`PreviousUpper`. Its failure is Eastmoney's single-page fetch showing through,
which is decision 5's separate Gate A.

### A second, latent stop

`Tonghuashun` serves only the upper pool and answers `Lower` and `Broken` with
`Unimplemented / capability_unadmitted`. BR-059 makes every non-retryable failure
terminal, so this candidate is a landmine for two of the three supported kinds:
as soon as a `Lower` or `Broken` pool exceeds the caller's `limit`, Eastmoney
returns incomplete, the route advances to Tonghuashun, and Tonghuashun's scope
decline stops the route before it reaches HithinkFinance. That path is
unreachable today only because both pools are currently smaller than the limits
used in these probes.

### The failure is not self-describing

The only log line emitted for the failure is:

```
level=ERROR target=grpc_server event=provider_route_failure
stage=provider_route_stopped request_id="..." operation=limit_pools attempt_count=1
```

It names neither the stopping Provider nor the attempt reason code; both exist
only in the binary trailer. Recovering "Eastmoney stopped the route with an
incomplete batch" required decoding `magic-error-detail-bin` by hand.

## Decision

### 1. A non-complete batch advances the route (amends BR-059)

`execute_limit_pool_route` keeps BR-059's termination rule for a **complete**
batch, including a truthful verified-empty batch. It gains a new rule for an
`Ok` batch that is not complete:

- record a bounded attempt for that candidate with outcome `rejected` and reason
  code `response_invalid`, both already inside the closed vocabulary that
  `ProviderAttempt::new` enforces (`ProviderAttempt` deliberately carries no
  free-form upstream text, so the batch's `quality().issues()` text stays in the
  Provider batch and out of the attempt trace);
- continue to the next candidate;
- if no candidate yields a complete batch, return
  `ProviderRouteFailure { exhausted: true }` with every attempt, so the caller
  can see that each candidate was tried and why each was unusable.

This is what BR-028 sentence 6 already requires of a whole-market consumer: it
requires `is_complete()`, so it must be allowed to take the first candidate that
can prove it. It does not weaken BR-059's closing guarantee — the route still
"never returns stale records, mixed provenance or partial success" — and it
strengthens the attempt trace, because an exhausted route now proves the whole
candidate list was tried.

Genuinely non-retryable faults keep stopping the route: `invalid_request`,
`unauthenticated`, `permission_denied`, and every Provider contract or evidence
violation other than the sanctioned truncation that BR-028 defines. The
`qdate`-divergence precondition observed on a non-trading date is such a fault
and is deliberately **not** reclassified here; whether a non-trading date should
resolve to a verified-empty batch is a separate contract question that needs its
own evidence round.

### 2. A scope decline is not a route stop (amends BR-059)

`ServiceError::Unsupported` from a candidate means "this Provider does not serve
this exact scope". That is a property of the candidate, not a defect in the
request, and it must not deny the caller the candidates that do serve it. The
route records `rejected` / `unsupported` and advances.

BR-059's existing sentence "Invalid requests, unsupported scopes, authentication
failures, response/evidence conflicts and other non-retryable failures stop the
route" is amended to separate the two: a scope that **every** candidate declines
still fails closed as that unchanged `Unsupported` error, so a caller asking for
a family no registered capability serves learns exactly that; a candidate that
declines one scope while another serves it no longer speaks for the operation.

### 3. Name the attempts in the route failure log

`provider_route_failure` gains the per-attempt Provider and reason code, in
registration order, passed through `safe_log_value` with the existing bounds. No
new response field and no new Protobuf field: the trailer already carries this,
and the log must not become a second, divergent contract. The purpose is that an
operator reading stderr can see which candidate stopped the route without
decoding a trailer.

### 4. The candidate scopes stay different, and that is the point

The three registered `LimitPools` scopes differ by Provider, and all three are
accurate:

| Provider | registered scope | families served |
| --- | --- | --- |
| Eastmoney | "upper, broken, lower and previous-upper pools for an exact trading date" | 4 |
| Tonghuashun | "upper-limit pool only for an exact trading date" | 1 |
| HithinkFinance | "official Fuyao Upper, Lower or Broken pool [...] `PreviousUpper` unsupported" | 3 |

BR-056's sentence "Production `LimitPools` supports only explicit-date `Upper`,
`Lower` and `Broken`. [...] `PreviousUpper` is unsupported because Fuyao exposes
no exact equivalent" sits under the heading "Official HITHINK Fuyao admission"
and constrains the **HithinkFinance** scope, which already records it. It is not
a statement about the operation, and Eastmoney does serve `PreviousUpper` through
`getYesterdayZTPool` (`crates/magic-eastmoney-rs/src/limit_pool.rs:27`) — a fact
`UpperLimitPoolReview` depends on, since its Eastmoney handler fetches all four
families, `PreviousUpper` included
(`crates/magic-market-composition/src/grpc_production.rs:3532`).

So no scope string changes. The union of the three scopes is why decision 2 is
required rather than optional: for `Lower` and `Broken`, exactly one of the three
candidates (Tonghuashun) declines, and for `PreviousUpper` exactly one
(Eastmoney) serves.

### 5. Eastmoney single-page fetch is a separate Gate A (out of scope)

The Eastmoney handler asks for one page sized to the caller limit, which is why
its batch is incomplete whenever the pool is larger. That is Eastmoney's
registered behaviour under BR-028, which sanctions the truncated batch, and not a
BR-056 violation — BR-056's "every declared page is fetched" sentence is
Hithink-scoped, and the HithinkFinance handler does page.

Making Eastmoney paginate would let it return complete batches and stay a
meaningful first candidate, but it changes the HTTP request pattern on a host
carried as a provider-local reviewed exception, so it requires its own Gate A
design and a matching `http-transports.tsv` update.

Decisions 1 and 2 restore the operation without it: the route advances to the
first candidate that can prove its batch whole, which for `Upper` is
`Tonghuashun` or `HithinkFinance` and for `Lower`/`Broken` is
`HithinkFinance`.

## Immediate mitigation (no code change)

After decisions 1 and 2, `LimitPools` callers need no workaround, and pinning
`preferredProvider=HithinkFinance` (which also returns `complete=true` for all
three kinds) remains available to anyone who wants to skip the fall-through.

`UpperLimitPoolReview` is unaffected by this design and still needs its caller to
set `per_pool_limit` at or above the pool size for the date. That is caller-side
and reversible, and decision 5's Gate A is what would remove the requirement.

## Rejected alternatives

- **Reorder the registrations so Eastmoney is last.** Hides the defect, leaves a
  handler advertised as admitted that cannot satisfy the operation's completeness
  contract, and still breaks on `Lower`/`Broken` at Tonghuashun's scope decline.
- **Withdraw the Eastmoney `LimitPools` admission.** Removes the only candidate
  besides HithinkFinance for `Lower` and `Broken`, and by itself breaks those two
  kinds, because Tonghuashun's `Unimplemented` would then become the terminal
  first attempt. It also discards a working Provider rather than the defective
  policy.
- **Treat an incomplete batch as a complete one.** Directly contradicts BR-028
  and BR-059, and would publish a truncated pool as the date's pool.
- **Make the route ignore `limit` and always request the full pool.** Changes the
  caller's request, which BR-059 forbids ("The exact request is unchanged on
  every attempt"), and lets a bounded consumer receive unbounded rows.
- **Retry the same candidate until it returns complete.** The incompleteness is
  deterministic in the caller's `limit`, so a retry cannot fix it, and it spends
  the Provider quota to prove that.
- **Reclassify the incomplete batch as retryable.** Would advertise a retryable
  fault that retrying cannot fix, and would still not reach the other candidates.

## Public contract

- Operation/RPC: `LimitPools`, `UpperLimitPoolReview` (names, request schemas and
  record schemas unchanged)
- Providers: `Eastmoney`, `Tonghuashun`, `HithinkFinance` (registration order and
  admission states unchanged)
- Changed behaviour: an unpinned `LimitPools` query whose first candidate returns
  a sanctioned incomplete batch, or declines the requested scope, now reaches the
  candidates that can answer it, and an exhausted route reports every attempt

## Verification

- Deterministic tests in `magic-market-service`: a first candidate returning
  `Ok(incomplete)` followed by a second returning `Ok(complete)` yields the
  second candidate's batch; all candidates incomplete yields
  `provider_route_exhausted` with one `response_invalid` attempt per candidate; a
  truthful verified-empty batch still terminates the route immediately; a first
  candidate returning `Unsupported` for the requested kind advances; a scope
  **every** candidate declines stays the unchanged `Unsupported` error;
  `invalid_request`, `unauthenticated` and `permission_denied` still stop the
  route; an explicit `preferred_provider` still never falls through.
- Every attempt in these tests stays inside the closed vocabulary and count
  bounds: `ProviderAttempt::new` rejects anything outside the contract, so a new
  reason code cannot be introduced silently.
- Log test: a route failure emits the attempt Providers and reason codes, and the
  line stays within the `safe_log_value` bounds.
- Registry: `tools/compliance/check_admissions.py` passes unchanged — no scope
  string, admission state or capability count is touched.
- Live, against the deployed binary on 2026-09-21, re-running the three cases
  recorded above as failing and one as passing
  (`target/runtime/limit-pools-route-fix-verify.ps1`):

  | kind | `limit` | before | after |
  | --- | --- | --- | --- |
  | `Upper` | 50 | fail `provider_route_stopped` | ok · `complete=true` · 50 rows · `Tonghuashun` |
  | `Broken` | 10 | fail `provider_route_stopped` | ok · `complete=true` · 10 rows · `HithinkFinance` |
  | `Lower` | 1 | fail `provider_route_stopped` | ok · `complete=true` · 1 row · `HithinkFinance` |
  | `Broken` | 25 | ok · `Eastmoney` | ok · `complete=true` · 25 rows · `Eastmoney` |

  The row count stays at the caller's `limit` because the serving Provider
  validates the whole pool and then applies that limit, which is the registered
  `LimitPools` semantics; `complete=true` is what certifies the pool behind those
  rows was whole. No `provider_route_failure` line was logged during the run.
- Live, unchanged by this design: `UpperLimitPoolReview` with
  `per_pool_limit=50` still fails with Eastmoney's truncation message, and with
  `per_pool_limit=103` and `500` returns `complete=true`.
- Gate C: formatting, workspace tests, Clippy, the repository compliance checks
  and the documentation link check. Note for whoever runs this on Windows:
  `cargo test --workspace --all-targets` cannot link there because several
  crates ship an example named `live_probe` writing to one output path;
  `cargo test --workspace --lib --bins --tests` is the equivalent that runs.

## Rollback

Revert the service-layer commit. The route returns to stopping on the first
non-complete batch, which restores the 2026-09-21 failure; the documented
caller-side mitigation (`preferredProvider=HithinkFinance`) remains available
throughout. No Provider, registry or transport state is touched by this design,
so the rollback is confined to one function and its tests.
