# Bounded server-side record for a failed-closed request

## Gate A boundary

This design adds one stderr record and one clause to BR-057. It changes
`status_from_error` and the blocking-worker seam in
`crates/magic-market-grpc-server/src/app.rs`. It adds no operation, no Provider, no
host, no transport, no admission state, no Protobuf field and no client-visible
value.

It exists because the downstream failure review of 2026-09-22 asked for exactly this
("建议服务端按能力 (operation) 维度的健康度自检 + 失败请求的 request_id 侧日志抽查"),
and because reproducing its two `internal` reports required the reviewer to hand us
their own timestamps and request shapes: a failed request could leave no server-side
trace at all.

## Problem

`status_from_error` maps every `ServiceError` to a gRPC status and records an event
for only three arms: `provider_failure`, `provider_route_failure` and
`provider_attempt_limit_exceeded`. Every other arm returns a status and writes
nothing.

For the arms whose caller-visible message is **scrubbed**, that is a hole with no
workaround. `ServiceError::Internal(_)` returns the fixed string
`"internal service error"` and never the message, so neither the caller nor an
operator can learn what broke:

```rust
ServiceError::Internal(_) => (
    Code::Internal,
    "internal",
    false,
    "internal service error".to_owned(),
    // ...  nothing is logged
),
```

The blocking-worker seam has the same shape: a `JoinError` from
`spawn_blocking` (a panic in a Provider worker) becomes
`Status::internal("blocking provider worker failed")` with no record.

Measured against the running server on 2026-09-22, **every** explicitly-pinned
failing request produced no server-side record. `LimitPools` pinned to a Provider
that refused the date, `Announcements` rejected at the boundary, and the internal
failure of R-08/R-09 all read `server_log: <none>`. The 19:00 review batch left the
server's stderr completely silent — the last record before it was 06:21Z.

That is the gap. An operator asked "what did the server do for this request ID?"
has no answer for any failure outside the three arms above.

## Decision

### 1. A failed-closed request writes exactly one bounded record

The arms that currently write nothing and whose failure is a **server-side** fact
gain one record each, through a single helper:

```rust
fn log_service_failure(request_id: &str, operation: Operation, reason_code: &str) {
    logging::event(
        Level::Error,
        "grpc_server",
        "service_failure",
        format_args!(
            "stage={} request_id={:?} operation={}",
            reason_code,
            safe_log_value(request_id, 128),
            operation.as_str(),
        ),
    );
}
```

The recorded arms are `source_precondition_failed` (`FailedPrecondition`),
`invalid_evidence` (`InvalidEvidence`), `internal` (`Internal`) and
`provider_unavailable` (`Unavailable`), plus the blocking-worker seam as
`stage=blocking_worker`.

`Unavailable` is in that list because of a measurement taken after the first
reproduction round, and it is the sharpest form of the gap. `provider_unavailable` is
the only reason code **two** arms produce, and only the other one
(`ProviderFailure { kind: Unavailable }`) recorded it, so the same client-visible code
came with a record or with silence depending on which arm fired. On 2026-09-22 an
Eastmoney transport outage that broke `ProviderTopNRankings`, `BoardFlows` and
`PostCloseFlows` ran for at least three hours and wrote **no** record: the server's
stderr for the whole day is 90 lines, none of them naming those operations. A
retryable `UNAVAILABLE` is precisely the failure an operator most needs to attribute,
because the caller is being told to retry against something that is not recovering.

`stage=` is the same field name `provider_failure` and `provider_route_failure`
already use for their reason code, so `grep 'stage=' ` reads as one vocabulary
across all server-side refusals, and `grep 'request_id="<id>"'` answers the
operator's question directly.

### 2. The record never carries the failure message

This is the constraint that shapes the change, and it is why the record is not
simply `message={:?}`.

BR-057 forbids unrestricted upstream text in logs. `ServiceError::Internal` messages
are built in ~30 places in `grpc_production.rs` by interpolating a serde or offset
error, so they are not guaranteed to be repository-authored text. The
`FailedPrecondition` message for the LimitPools case is literally
`"Eastmoney protocol error: limit-pool source qdate 2026-09-22 does not match
requested date 2026-09-21"` — Provider-derived text.

So the record carries the reason code, the request ID and the operation, and
deliberately **not** the message. What an operator needs to do their job is the
correlation key and the classification; both are bounded, low-cardinality and
repository-authored. The upstream text stays where BR-057 keeps it: in the typed
error the caller already receives (for `FailedPrecondition` and `InvalidEvidence`)
or in no place at all (for `Internal`, whose message is already scrubbed for the
caller and stays scrubbed for the log).

### 3. Client-error arms stay silent

`invalid_request`, `unsupported`, `unauthenticated`, `permission_denied`,
`resource_exhausted` and `deadline_exceeded` keep writing nothing.

These are request outcomes the caller owns, the caller already receives the message
verbatim, and logging one record per malformed request would let a single
misbehaving client turn its own errors into the server's log volume. The line is:
the server records what it failed **at**, not what the caller asked for wrongly.

### 4. Telemetry stays off the decision path

The helper is a fire-and-forget `logging::event` call placed inside an arm that has
already computed its tuple. It reads no shared state, allocates nothing that
outlives the call, and cannot alter the returned code, reason code, retryable flag,
admission state, completeness, routing or fail-closed behavior — BR-057's existing
requirement, restated here because it is the property a reviewer should check.

## Cost

One record per failed-closed request, on the error path only. Successful queries
are untouched, so BR-057's "successful queries do not emit per-request logs" holds
unchanged. No new metric, exporter, listener, queue, background task or lock.

## Tests

`logging::event` writes to the process stderr and has no injectable sink, so the
record body is split out as `service_failure_record` and tested directly. In
`crates/magic-market-grpc-server/src/app.rs`:

- `service_failure_record_names_the_request_operation_and_stage` — exact record
  equality, including the bounded and control-character-stripped request ID.
- `internal_failure_is_scrubbed_for_the_caller_and_never_logged_verbatim` — the
  regression test for decision 2. It asserts the caller's message is scrubbed, and
  that a record built for a failure whose message is Provider-derived text contains
  neither that text nor the `qdate` fragment of it.
- `recorded_service_failure_arms_keep_their_caller_visible_status` — pins decision 4
  for `FailedPrecondition` (code, message, `reason_code`, `retryable`) and records
  the unchanged `InvalidRequest` status that decision 3 leaves silent.
- `provider_unavailable_is_recorded_from_both_arms_that_produce_it` — asserts the
  `Unavailable` arm's caller-visible status is unchanged (`UNAVAILABLE`, the reason
  verbatim, `retryable=true`) and that its bounded record contains neither the
  endpoint nor the upstream error text.

Decision 3's "client-error arms write nothing" is a **structural** property, not a
unit-testable one: the arms do not call the helper, and no test can observe the
absence of a stderr write. It is pinned by review of the call sites, and the count is
the check — `log_service_failure` has exactly five callers, the ones listed in
decision 1.
