# Coverage-sandbox transport test design

## Scope

This Gate A repair removes loopback-listener assumptions from upstream
transport tests. Production TCP/HTTPS adapters, endpoint allowlists, rate
limits, source parsing and public provider interfaces remain unchanged.

## Root cause

The authoritative coverage runner may execute instrumented tests without
permission to bind a local listening socket. Three fixtures failed before
reaching production code:

1. a TDX pool lifecycle regression;
2. two TDX connection regressions;
3. one Tonghuashun concrete-HTTPS regression.

A repository-wide Rust source audit found no other `TcpListener`,
`127.0.0.1:0`, or `bind()` fixture after the TDX repair. The remaining THS
fixture duplicates a seam already used by CNInfo: map an in-memory
`ureq::Response` or typed `ureq::Error` through the concrete transport result
collector.

## Design

- TDX retains `TcpConnection` as its external interface and `TcpStream` as its
  production adapter. A private stream/connector seam makes timeout and byte-I/O
  behavior deterministic in package tests.
- THS extracts `collect_transport_result`, used unchanged by the production
  `HttpsTransport::execute` path. Tests construct in-memory 200/403 ureq
  responses and a deterministic invalid-URL transport error.
- Real network behavior stays covered by provider live probes. Unit/coverage
  tests prove status/error/body mapping without requiring a local server.
- Tests are replaced only after their production branches have equivalent or
  stronger deterministic assertions. No test is ignored and no coverage
  threshold changes.

## Failure modes

- Invalid URL, connect/TLS/DNS, or ureq transport errors remain explicit typed
  provider transport errors.
- Non-2xx HTTP responses retain their real status and bounded body for the
  provider admission layer.
- Oversized, missing-media-type, redirect and schema failures remain covered by
  the existing bounded reader and response-validation tests.

## Old module relation

All production providers and transports are retained. This is a testability
repair at existing internal seams, not a second transport implementation or a
fallback.

## Validation

- repository-wide loopback-fixture source audit returns no matches;
- targeted TDX and THS transport tests;
- workspace formatting, strict Clippy, full tests and compliance;
- authoritative workspace coverage and threshold checker;
- remote live workflow for real HTTPS/TCP evidence.

## Rollback

Revert the transport-testability commit. The old tests may then remain unusable
in restricted coverage runners; production provider behavior is otherwise
unchanged.
