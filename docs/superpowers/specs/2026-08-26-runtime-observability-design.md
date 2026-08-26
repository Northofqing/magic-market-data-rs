# Runtime observability design

## Gate A decision

Runtime observability lives at the two existing deep seams instead of inside
Provider implementations:

1. Every market-data unary request already crosses
   `GrpcApplication::query`. Aggregate query counters and latency are recorded
   once at that seam.
2. Every local-terminal event and Agent lifecycle transition already crosses
   `EventHub` while its bounded state lock is held. Replay and Agent counters
   are updated inside that existing critical section.

No Provider, Router, Core record, admission decision, evidence value, URL or
transport policy changes. Operational timestamps are never source evidence.

## Log contract

Production stderr records start with a UTC RFC3339 timestamp and stable
low-cardinality fields:

```text
ts=2026-08-26T03:12:45.123456789Z level=ERROR target=grpc_server event=provider_failure ...
```

Secrets, authorization metadata, payload bodies and unrestricted upstream text
remain prohibited. Existing diagnostic fields such as `stage`, `request_id`,
`operation`, `provider` and the bounded safe Provider reason remain available.

TDX logging keeps its runtime level check. Timestamp formatting and message
formatting happen only after the level is enabled. Successful high-frequency
polls and successful unary calls do not write logs.

## Aggregate query telemetry

`GetHealth` appends a backward-compatible `RuntimeObservability` message with:

- process start and monotonic uptime;
- started, succeeded, failed, cancelled and in-flight query counts;
- rejected and timed-out query counts;
- total and maximum query duration in microseconds;
- unary and blocking concurrency limits and currently available permits.

One query uses a monotonic clock plus relaxed atomic updates. The observation
guard records cancellation on drop, so cancelled futures cannot leak the
in-flight gauge. Metrics are process-lifetime aggregates and carry no
high-cardinality labels.

## Event telemetry

`GetListenerStatus` appends the oldest replay cursor, current replay event/byte
counts, active subscriber count, cumulative Agent connections/disconnections,
published events and replay evictions. Updates reuse the existing `EventHub`
lock and add no lock, task, queue, network listener or disk write.

## Performance boundary

- No per-success logging.
- No payload serialization for metrics.
- No histogram allocation or label map.
- No new background task or exporter.
- Health/status serialization occurs only when those RPCs are called.
- Event counters are plain checked/saturating integer updates under an already
  required lock; query counters use relaxed atomics.

## Verification

- deterministic timestamp-format tests;
- query success/failure/timeout/cancellation and in-flight tests;
- listener-status counter and replay-window tests;
- protobuf compatibility and documentation checks;
- focused tests plus workspace formatting, Clippy, compliance and docs checks.
