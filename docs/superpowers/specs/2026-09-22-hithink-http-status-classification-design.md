# HITHINK rejected-HTTP-status classification design

## Gate A boundary

This design is written on the caller's 2026-09-22 instruction to fix the HTTP 429
misreport in `map_hithink_error`.

It changes one thing: how a **rejected HTTP status** on a Fuyao call is
classified. It adds no admission, moves no `*_ADMITTED` constant, and changes no
request schema, record schema, protobuf field, RPC or endpoint. It does not widen
any HTTP host, path, timeout, body-size, redirect, proxy or authentication
policy, and it reuses only the existing allowlisted Fuyao host and paths.

It adds **no new dependency edge**. In particular `magic-market-composition`
still does not depend on `magic-market-transport`, so
[`http-transports.tsv`](../../integrations/http-transports.tsv) is untouched.

What it changes is the mapping table that [`hithink-fuyao.md`](../../integrations/hithink-fuyao.md)
already publishes, and with it two client-visible classifications
(`429` and the non-5xx statuses) plus their `provider_attempts` reason codes.

It does not decide anything new. BR-056 already requires that "key absence,
expiry, authentication/permission denial, rate limiting, upstream unavailability,
query rejection and invalid provider responses are **distinct typed terminal
outcomes**". This design makes the implementation match that registered rule
instead of collapsing four of those outcomes into one.

## Problem

`map_hithink_error` (`crates/magic-market-composition/src/grpc_production.rs:5222`)
destroys the HTTP status before anything can classify it:

```rust
HithinkError::Transport(_) => (
    ProviderFailureKind::Unavailable,
    "category=transport".into(),
),
```

The status is present one layer down: `HithinkError::Transport(#[from] TransportError)`
and `TransportError::HttpStatus { status: u16 }`
(`crates/magic-market-transport/src/lib.rs:23`). The transport rejects every
non-`200`, non-`3xx` response through that one variant
(`crates/magic-market-transport/src/http.rs:370-379`), so a `401`, a `429`, a
`503` and a `404` all arrive at this arm, and all four leave it as
`provider_unavailable`.

Three consequences, in increasing order of severity:

1. **The client gets the wrong code.** A throttle is reported as an outage, so a
   caller that follows [`grpc-external-api.md`](../../integrations/grpc-external-api.md)
   §11 backs off and re-checks health for something that only needed pacing.
2. **The operator cannot diagnose it.** `provider_reason` is the only place the
   provider-side detail survives, and it says `category=transport` for every
   case. The exact status reaches neither the gRPC boundary nor
   `target/runtime/logs/grpc-server.stderr.log`.
3. **The provider's own taxonomy is inconsistent.** HITHINK returns rate
   limiting **in band** as envelope `code=4001`, which already maps to
   `RateLimited` / `provider_rate_limited`
   (`grpc_production.rs:5206`). The same provider throttling the same client at
   the HTTP layer produces a different, wrong kind. One provider, one condition,
   two answers depending on which layer noticed.

The published mapping table has the same defect in documentation form: it has no
row for a rejected status and folds it into "network failure →
`provider_unavailable`".

## Observed evidence

### The status is reached and is discarded

A bounded live probe on 2026-09-22 ran the repository's own example,
`cargo run -p magic-hithink-rs --example auction_observation_probe final 3`
(`crates/magic-hithink-rs/examples/auction_observation_probe.rs`). It printed two
successful calls and then the typed transport error carrying the status:

```
call=1 stage=Final records=2 observed_at=unix-ms:1790042833128 source_at_absent=true
call=2 stage=Final records=2 observed_at=unix-ms:1790042833427 source_at_absent=true
Error: Transport(HttpStatus { status: 429 })
```

The same period's gRPC service log records what the *service* did with Fuyao
failures. Ten `HithinkFinance` / `current_auction_observations` failures that day
all read:

```
level=ERROR target=grpc_server event=provider_failure stage=provider_unavailable
  request_id="auction-final-1" operation=current_auction_observations
  provider="HithinkFinance" provider_reason="category=transport"
```

`stage=provider_unavailable` is the reason code for
`ProviderFailureKind::Unavailable`. So a status the transport had already typed as
`429` was reported at the boundary as "provider is unavailable" with the
diagnostic `category=transport`.

### Why the discarded status is a rate limit, not an outage

- The probe's two preceding calls **succeeded**, and earlier in the same session a
  failing call **succeeded on retry after roughly five seconds**. An upstream that
  is genuinely unavailable does not recover on that timescale, three times over.
- The provider publishes a metered quota and returned the in-band equivalent,
  `code=5003`, in the same window.
- After the change, this question stops being an inference: `provider_reason`
  carries `http_status={status}`, so the next occurrence identifies itself.

Recorded limitation, because it affects how much this evidence proves: the 429
was observed after roughly ten Fuyao calls in one hour, and this session's own
probing may have contributed to it. That does not weaken the defect statement —
the arm discards the status regardless of which status arrives — but it does mean
the observation is of a self-inflicted, transient throttle rather than of an
independent provider-side outage.

### The discard is verifiable from the code alone

The evidence above is not what establishes the defect; the code does. A `429`
that reaches `TransportError::HttpStatus` reaches the `HithinkError::Transport(_)`
arm, and that arm cannot distinguish `429` from `503`. Nothing in the evidence
chain is needed to see that.

## Decision

### 1. Carry the status as a typed fact

Add to `HithinkError` (`crates/magic-hithink-rs/src/lib.rs`), beside the existing
`Transport` variant:

```rust
#[error("HITHINK request rejected with HTTP status {0}")]
HttpStatus(u16),
```

The tuple form matches the crate's own single-field variants and the four
providers that already carry this fact: `ClsError::HttpStatus(u16)`,
`CninfoError::HttpStatus(u16)`, `ThsError::HttpStatus(u16)`,
`ExchangeError::HttpStatus(u16)`.

`Transport` keeps its `#[from] TransportError` and keeps every other transport
failure — network, TLS, redirect, media type, resource limit, internal. Only the
status is lifted out.

### 2. Translate at the provider boundary

Fuyao's client already translates transport outcomes into typed provider errors
at one seam, `HithinkClient::execute_json`. Give that seam the same treatment
`magic-exchange-rs` gives its transport
(`crates/magic-exchange-rs/src/transport.rs:196-200`):

```rust
fn execute_json<T: DeserializeOwned>(&self, request: &HttpRequest) -> Result<Success<T>, HithinkError> {
    self.execute_request(request).map_err(map_transport)
}

/// Retains a rejected HTTP status so callers can classify it.
fn map_transport(error: TransportError) -> HithinkError {
    match error {
        TransportError::HttpStatus { status } => HithinkError::HttpStatus(status),
        other => HithinkError::Transport(other),
    }
}
```

The existing body moves to `execute_request` unchanged. This is deliberately
total: every `TransportError` leaving the request seam passes through one
function, so no future `?` can silently reintroduce a raw status.

### 3. Classify the status exactly as the registered CLS policy does

In `map_hithink_error`, add one arm that mirrors `map_cls_error`
(`grpc_production.rs:4995-5003`) statement for statement:

```rust
HithinkError::HttpStatus(status) => {
    let kind = match status {
        401 | 403 => ProviderFailureKind::AuthenticationRejected,
        429 => ProviderFailureKind::RateLimited,
        500..=599 => ProviderFailureKind::Unavailable,
        _ => ProviderFailureKind::QueryRejected,
    };
    (kind, format!("http_status={status}"))
}
```

The reason string is `http_status={status}`, the same spelling the CLS mapper
already emits for the same fact. The `Transport` arm keeps `category=transport`.

This is not a new policy. It is the policy the repository registered for
`ClsError::HttpStatus`, applied to the same fact from a second provider, so that
one HTTP status has one meaning at the boundary.

### 4. Correct the published table

`hithink-fuyao.md`'s mapping table gains the status rows and stops describing a
rejected status as a "network failure". Nothing else in that document changes:
the in-band rows (`2001`, `2003`, `4001`, `1001..1004`, `3001`, `3004`, `3002`,
`5001..5003`) keep their codes, kinds and retryability.

### 5. Record the change where clients can see it

`grpc-external-api.md` is a bundled client contract document, and the change is
observable to a client that hits a throttle. Its version history gains a
`2026-09-22.1` entry, and the bundle baseline follows in the two places that
carry it (`tools/docs/build_client_bundle.ps1`'s `$BundleVersion` default and
`README.md`), exactly as `2026-08-24.2` recorded the analogous TDX
connection-error classification.

The closed reason-code set does not change: `provider_rate_limited`,
`provider_authentication_rejected`, `external_query_rejected` and
`provider_unavailable` are all already in it, and `rate_limited`,
`authentication_rejected` and `query_rejected` are already in the
`provider_attempts` closed set.

## Behaviour change

Everything below is client-visible and is the whole point of the change.

| Rejected outcome | Today | After |
| --- | --- | --- |
| HTTP `401`, `403` | `UNAVAILABLE` / `provider_unavailable` / retryable | `PERMISSION_DENIED` / `provider_authentication_rejected` / **not** retryable |
| HTTP `429` | `UNAVAILABLE` / `provider_unavailable` / retryable | `RESOURCE_EXHAUSTED` / `provider_rate_limited` / retryable |
| HTTP `500..=599` | `UNAVAILABLE` / `provider_unavailable` / retryable | unchanged |
| Any other rejected status (`1xx`, `2xx` other than `200`, other `4xx`) | `UNAVAILABLE` / `provider_unavailable` / retryable | `FAILED_PRECONDITION` / `external_query_rejected` / **not** retryable |
| Other transport failure | `UNAVAILABLE` / `provider_unavailable` / retryable | unchanged |
| Envelope `4001` | `RESOURCE_EXHAUSTED` / `provider_rate_limited` / retryable | unchanged |
| Envelope `2001`, `2003`, `1001..1004`, `3001`, `3004`, `3002`, `5001..5003` | *(documented)* | unchanged |

Two consequences deserve to be stated rather than discovered:

- **Retryability changes for two rows.** A `429` and a `5xx` stay retryable, so
  the overwhelmingly common throttled path keeps its behaviour; but a rejected
  credential and an unexpected status become terminal. That is the registered
  rule, not a side effect: BR-059 states that "authentication and permission
  failures" stop a route and that "retryable availability, timeout and
  rate-limit failures advance as before".
- **Route attempts change their reason code for the same reason.** A throttled
  `HithinkFinance` candidate moves from
  `{"outcome":"failed","reason_code":"unavailable","retryable":true}` to
  `{"outcome":"failed","reason_code":"rate_limited","retryable":true}` — still a
  retryable `failed` attempt, so the candidate still advances under BR-059. A
  `401`/`403` candidate moves to `authentication_rejected` and now **stops** the
  route. Both reason codes are already inside the closed `provider_attempts`
  contract, so no trace ever becomes uninterpretable.

## Rejected alternatives

- **Expose `HithinkError::http_status() -> Option<u16>` and classify in the
  composition crate.** This was the first design. It avoids touching
  `HithinkError`, but `magic-market-composition` cannot name `TransportError` to
  construct the input, so the new classification arm would ship untested — or it
  would need a new `magic-market-transport` dependency edge on the composition
  crate, which is a governed transport change for no benefit. The typed fact also
  matches what four providers already do.
- **Keep `Transport(_)` and widen the composition mapper to guess.** The
  information is gone by then. Guessing "transport failure that is probably a
  throttle" is precisely the fabrication Gate B forbids.
- **Map a rejected status onto the existing `RateLimited { request_id }`.** That
  variant carries an in-band envelope request id. An HTTP status has none, so
  this would either invent one or leave a field that lies. The two facts are
  distinct and now stay distinct.
- **Classify only `429` and leave `401`/`403` and the remaining statuses as
  transport.** An arbitrary split of one fact, and it would leave
  `ProviderFailureKind::QueryRejected` unreachable for a provider that plainly
  returns `4xx`. Mirroring the registered CLS policy is simpler to state and to
  verify.
- **Classify the remaining transport variants too** (`Redirect`, `MediaType`,
  `ResourceLimit`). Deferred, not refused: those are request- and response-shape
  policy faults whose registered treatment is genuinely unsettled — `magic-sec-rs`
  deliberately keeps `429` as transport at all. This design classifies one fact
  and leaves the rest exactly as they are.
- **Change the `Transport` arm to non-retryable.** Would make a network blip
  terminal and contradicts BR-059 and the `UNAVAILABLE` client guidance.

## Public contract

- Operation/RPC: none renamed, added or removed; no protobuf change.
- Provider registration and every `*_ADMITTED` constant: unchanged.
- Transport: no new host, path, timeout, body-size, redirect, proxy or
  authentication policy; no new dependency edge.
- Closed sets: `provider_attempts` and provider-failure reason codes are
  unchanged as sets. Only which member a given rejected status selects changes.
- Changed: the classification of a rejected Fuyao HTTP status, the
  `provider_reason` spelling for it (`category=transport` → `http_status={status}`),
  the `hithink-fuyao.md` mapping table, and the client-bundle baseline
  (`2026-09-17.1` → `2026-09-22.1`).

## Verification

- Registry: `tools/compliance/check.sh` passes; no `admissions.tsv` row and no
  `*_ADMITTED` constant is touched, so `check_admissions.py` is unaffected by
  construction and is run to prove it.
- Provider: a `magic-hithink-rs` test drives a transport that returns a rejected
  status and asserts it surfaces as `HithinkError::HttpStatus(429)` rather than
  `Transport(_)`, and that a non-status transport failure still surfaces as
  `Transport(_)`.
- Composition: a test in the existing
  `provider_failures_preserve_retry_and_precondition_categories` covers
  `401`/`403` → `AuthenticationRejected` with `http_status=401`,
  `429` → `RateLimited` with `http_status=429`, `503` → `Unavailable`, and a
  non-5xx status → `QueryRejected`, mirroring the `ClsError::HttpStatus(429)`
  assertion beside it.
- Gate C: `cargo fmt --all -- --check`,
  `cargo test --workspace --lib --bins --tests`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `tools/compliance/check.sh`, and `tools/docs/check_links.sh`.
- Documentation: `hithink-fuyao.md`, `grpc-external-api.md` and this design agree
  on the four status rows; the link check passes.
- Not part of this gate: a live end-to-end capture of `http_status=429` through
  the gRPC boundary requires the service running against a live throttle, which
  cannot be scheduled. The change makes that capture possible; it is not required
  to prove the classification, which the tests pin deterministically.

## Rollback

Revert the code commit: `HithinkError::HttpStatus` disappears, `execute_json`
returns to the single-body form, and the composition arm returns to
`Unavailable` / `category=transport`. Revert the documentation commit to restore
the previous mapping table and bundle baseline. No admission state, contract or
transport policy is involved, so nothing else needs unwinding.
