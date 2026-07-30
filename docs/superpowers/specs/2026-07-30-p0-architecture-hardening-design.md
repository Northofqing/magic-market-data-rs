# P0 Architecture Hardening Design

**Status:** Proposed for user review  
**Date:** 2026-07-30  
**Scope:** synchronous TDX pool concurrency, blocking-runtime documentation, and HTTP transport-boundary governance

## Intent

This change removes one proven synchronous concurrency defect, makes the
blocking nature of every HTTP Provider explicit to async consumers, and stops
the workspace from accumulating additional unregistered Provider-local HTTP
stacks while the existing stacks are migrated separately.

The change preserves ordered Router semantics, all four approved TDX client
strategies, current Provider capability admission, typed failures, provenance,
and the rolling-stable toolchain policy.

## Scope choice

The architecture audit spans several independent programs. Three implementation
approaches were considered.

### Approach A: refactor every transport and Router in one release

This would replace all ureq clients, add async routing, change error
classification, and alter public APIs together. It has the shortest theoretical
path to one stack but combines unrelated failure domains and would require
re-running every Provider's deterministic and live evidence at once.

This approach is rejected.

### Approach B: bounded P0 hardening slice

This approach fixes the proven TDX lock-lifetime defect, documents the current
blocking contract, and adds a mechanically checked inventory that prevents new
private HTTP stacks. Existing Provider transports remain functional and can be
migrated one family at a time under separate Gate A specifications.

This is the selected approach.

### Approach C: TDX hotfix only

This would be the smallest code change, but it would leave the HTTP split free
to grow and would leave async service users without essential blocking-runtime
guidance.

This approach is rejected.

## 1. Synchronous TDX pool concurrency

### Root cause

`TdxHqClient::try_send_and_recv` obtains a
`MutexGuard<Arc<ConnectionPool>>`, calls `borrow` through that guard, and keeps
the resulting `PooledConnGuard` alive through socket send and receive.
`PooledConnGuard` borrows the `ConnectionPool` reached through the outer mutex
guard, so the outer pool-handle mutex remains locked for the whole request.

The inner `ConnectionPool` supports five active connections under BR-003, but
the outer handle mutex can serialize all synchronous requests before they reach
the inner pool.

### Repair

The request path will:

1. lock the outer pool-handle mutex;
2. clone the `Arc<ConnectionPool>`;
3. release the outer mutex;
4. borrow a connection from the cloned `Arc`;
5. perform send, header read, body read, and decompression as before.

The outer mutex remains necessary because connect/disconnect lifecycle code can
replace or close the current pool handle. Cloning the `Arc` preserves the
selected pool generation for the complete request, while BR-029 continues to
decide whether an active guard may be returned after `close_all`.

No public type or method changes.

### Regression proof

A private deterministic loopback test will configure a two-connection pool and
start two `send_raw_and_recv` calls on the same `TdxHqClient`.

The loopback server will:

1. accept and read the first request;
2. observe, within a bounded condition loop, whether the second request arrives
   before replying to the first;
3. reply to the first request even when the second has not arrived, so the old
   serial implementation cannot hang the test;
4. accept and reply to the second request;
5. report whether both requests were concurrently in flight.

Each response is a valid 16-byte TDX response header declaring a one-byte body,
followed by that body byte. The test requires both client calls to succeed and
requires the server to have observed the second request before the first
response. It fails on the current lock lifetime and passes after the `Arc`
clone repair.

## 2. Blocking-runtime contract

### Public integration guide

Create `docs/integrations/async-blocking.md` and link it from the root README and
the integrations index.

The guide will state:

- `magic-market-transport::ReqwestTransport` uses `reqwest::blocking`;
- current public-web Provider clients use blocking reqwest or ureq;
- `RequestGate::wait_for_turn` may sleep the calling thread;
- these calls must not run directly on a Tokio executor worker;
- async services should clone the client, move owned request data into
  `tokio::task::spawn_blocking`, and handle both the join error and Provider
  error;
- callers remain responsible for a bounded blocking concurrency budget, such
  as a semaphore or a service-level queue;
- `spawn_blocking` does not make an in-flight blocking socket request
  cancellable, so Provider timeouts remain mandatory.

The example will be compile-oriented pseudocode tied to the existing
`TencentClient` and `RealtimeQuotes` contract. It will not add Tokio to any HTTP
Provider crate.

### Contributor and agent guidance

`CONTRIBUTING.md` will describe release preflight as using the runner's current
default/rolling-stable toolchain with locked offline dependencies. It will not
claim a pinned minimum toolchain.

`AGENTS.md` will link the engineering rules, business rules, transport registry,
and async/blocking guide. It will instruct changes not to add a Provider-local
HTTP client or weaken endpoint policy without an approved Gate A design.

## 3. HTTP transport-boundary registry

### Registry

Create `docs/integrations/http-transports.tsv` with these columns:

```text
crate	mode	direct_dependencies	shared_transport	migration_status	reason
```

The registry covers every workspace crate whose production dependencies include
one of:

- `reqwest`
- `ureq`
- `rustls`
- `native-tls`
- `ring`
- `magic-market-transport`

Column rules:

- `mode` is one of `infrastructure`, `shared`, `legacy-direct`, or `hybrid`;
- `direct_dependencies` is a comma-separated, sorted list from the direct HTTP
  dependency set, or `-`;
- `shared_transport` is `true` or `false`;
- `migration_status` is `target`, `legacy`, or `reviewed-exception`;
- `reason` is `-` only for target shared/infrastructure rows; legacy and
  exception rows require an explicit bounded explanation.

Mode invariants:

- `infrastructure` is reserved for `magic-market-transport`;
- `shared` has `shared_transport=true` and no direct HTTP dependencies;
- `legacy-direct` has at least one direct HTTP dependency and does not depend on
  `magic-market-transport`;
- `hybrid` has both a direct HTTP dependency and
  `magic-market-transport`.

The registry records architecture state; it does not claim live capability
admission.

### Compliance checker

Create `tools/compliance/check_http_transports.py` using Python's standard
library. It will parse tracked workspace manifests, discover the production
dependency facts above, read the TSV, and fail when:

- a discovered crate has no registry row;
- a registry row has no corresponding discovered crate;
- a mode contradicts the manifest;
- direct dependency names differ from the manifest;
- a legacy/exception row lacks a reason;
- a new direct HTTP dependency appears without review;
- a path is untracked, missing, outside the repository, or a symbolic link.

The checker is read-only. It does not rewrite manifests or the registry.

Unit tests will exercise valid shared/direct/hybrid rows plus missing,
duplicate, stale, contradictory, and malformed rows in temporary repositories.

`tools/compliance/check.sh` will require the registry and guide, then invoke the
new checker. `docs/integrations/README.md` will link the registry without
presenting it as an admission matrix.

## Error and failure behavior

- TDX request errors remain the existing typed `TdxError` variants.
- Pool lock poisoning remains explicit through the existing `sync::lock`
  helper.
- The transport checker prints one `HTTP transport boundary error:` line per
  violation and exits non-zero.
- No compliance or documentation path silently repairs drift.
- No live probe result, capability constant, or provenance field changes.

## Compatibility

This slice adds no runtime dependency and makes no public Rust API change.
Existing Provider clients keep their current transport traits and constructors.
Existing ureq and shared-transport behavior remains unchanged.

The registry intentionally permits the currently reviewed legacy split. A
separate Provider-family migration specification must update both the manifest
and registry and must supply the relevant deterministic and live evidence.

## Verification

Focused verification:

```bash
cargo test -p magic-tdx-rs net::client::tests::blocking_pool_allows_two_in_flight_requests --locked --offline
python3 -m unittest tools.compliance.test_check_http_transports
python3 tools/compliance/check_http_transports.py
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
```

Release verification follows the repository gates:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets --all-features --locked --offline -- --test-threads=1
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked --offline
cargo test --workspace --all-features --doc --locked --offline -- --test-threads=1
bash tools/compliance/check.sh
bash tools/docs/check_links.sh
```

No live probe is required because this slice does not change an endpoint,
parser, capability, or admission claim.

## Rollback

The TDX repair, documentation correction, and transport-boundary registry are
independent commits. Each can be reverted without rewriting evidence history.
Reverting the registry checker reopens architecture drift but does not alter
runtime behavior. Reverting the TDX repair restores synchronous serialization
and must also revert its concurrency claim and regression test.
