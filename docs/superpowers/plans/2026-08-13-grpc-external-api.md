# gRPC External API and TDX Listener Implementation Plan

**Status:** transport complete; 46 production provider bindings registered and 8 exact fail-before-I/O blockers retained
**Design:** `docs/superpowers/specs/2026-08-13-grpc-external-api-design.md`

## Objective

Deliver a versioned, bounded gRPC surface for every current read-only Core data
family and for TDX local-terminal listener events, while preserving provider
admission, provenance, blocking isolation and the account/trading exclusion.

## Task 1 — Gate A contracts and registries

- Add BR-045 and the reviewed gRPC service registry.
- Record exact dependency, bind, TLS/auth, size, concurrency and streaming
  boundaries.
- Link the separate TDX listener design without changing its data admissions.

## Task 2 — Protobuf contract crate

- Add `magic-market-grpc-contracts` with checked-in `.proto`, vendored-protoc
  build and descriptor set.
- Define System, MarketData, MarketEvent and TdxAgent services.
- Enumerate every current provider-neutral read family.
- Add descriptor, enum exhaustiveness, request-bound and golden-wire tests.

## Task 3 — Application service crate

- Add `magic-market-service` as a synchronous, gRPC-independent facade.
- Define exhaustive operations, validated canonical requests/responses, typed
  errors and capability inventory.
- Implement admission-before-I/O and bounded production Provider composition.
- Keep unavailable families callable only as explicit typed failures.

## Task 4 — gRPC server

- Add bin-only `magic-market-grpc-server` using Tonic/Tokio.
- Implement unary methods, deterministic status mapping, health and optional
  reflection.
- Add explicit bind/TLS/auth/resource configuration and bounded
  `spawn_blocking` isolation.

## Task 5 — Event hub

- Implement positive injected replay and subscriber limits without defaults.
- Enforce generation/sequence and explicit loss/reset semantics.
- Implement listener status, replay and server-streaming subscription tests.

## Task 6 — Windows TDX agent

- Add bin-only `magic-market-tdx-agent`.
- Supervise the co-located existing diagnostic monitor, read bounded framed
  events, correlate terminal generation/sequence and connect outward via gRPC.
- Preserve diagnostic/admitted=false state and prohibit arbitrary helper,
  endpoint and trading commands.

## Task 7 — Provider coverage

- Wire all admitted production compositions to their corresponding unary RPCs.
- Return typed unimplemented for unadmitted or not-yet-composed families before
  I/O and expose the exact blocker through GetCapabilities.
- Add fake-backend coverage for every method and live evidence only where the
  underlying Provider already permits it.

## Task 8 — Deployment and release

- Package the cross-platform server and Windows TDX agent/monitor/discovery
  siblings with manifest hashes.
- Document generated clients, local and remote startup, secret injection and
  operational limits.
- Add Windows/Linux CI compile and deterministic in-process E2E.

## Task 9 — Full verification

- `cargo fmt --all -- --check`
- crate and workspace tests, Clippy and docs
- gRPC contract compatibility and cross-language fixture tests
- compliance, registry, links, audit/deny and release preflight
- bounded Windows live TDX stream and reconnection evidence

## Completion rule

Interface coverage is complete only when every Core read family has a registered
RPC with a deterministic production result or a fail-before-I/O typed blocker.
Transport completion does not change Provider or anomaly admission.
