# Phase 3: Transport and Four Client Strategies Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver bounded blocking and Tokio transport plus distinct, observable, cancellation-safe Blocking, Direct, Async, and Smart client strategies over a shared strict protocol execution contract.

**Architecture:** A protocol request becomes an immutable `RequestEnvelope` and is executed under one operation deadline. Blocking uses a five-connection pool, Direct creates one connection per request under a semaphore, Async owns four connection tasks with bounded channels and deterministic round-robin, and Smart selects/fails over observable endpoints while consuming one shared retry budget.

**Tech Stack:** Rust stable, std TCP, Tokio net/sync/time, tracing, parking_lot, crossbeam-channel, socket2, local scripted loopback servers, Criterion.

---

## Exit gate

Phase 3 is complete only when every client has deterministic loopback evidence for success, malformed response, timeout, disconnect, cancellation/drop, queue/pool saturation, retry budget, and limiter behavior. No default test contacts a public TDX endpoint; no client silently returns empty data or an unreadable connection to a pool.

### Task 1: Lock validated configuration, endpoints, deadlines, and response contracts

**Files:**
- Create: `crates/magic-tdx-rs/src/config.rs`
- Create: `crates/magic-tdx-rs/src/transport/mod.rs`
- Create: `crates/magic-tdx-rs/src/transport/endpoint.rs`
- Create: `crates/magic-tdx-rs/src/transport/response.rs`
- Create: `crates/magic-tdx-rs/tests/config.rs`
- Modify: `crates/magic-tdx-rs/src/lib.rs`
- Modify: `crates/magic-tdx-rs/Cargo.toml`

- [ ] **Step 1: Write failing configuration tests**

```rust
use std::time::Duration;
use magic_tdx_rs::{ClientConfig, Endpoint, ErrorKind, RateLimitScope};

#[test]
fn configuration_rejects_unbounded_or_zero_resources() {
    let endpoint = Endpoint::new("loopback", "127.0.0.1", 7709).unwrap();
    assert!(matches!(ClientConfig::builder().endpoint(endpoint.clone()).pool_size(0).build(), Err(error) if error.kind() == ErrorKind::Configuration));
    assert!(ClientConfig::builder().endpoint(endpoint).overall_timeout(Duration::ZERO).build().is_err());
}

#[test]
fn defaults_are_documented_and_bounded() {
    let config = ClientConfig::builder().endpoint(Endpoint::new("loopback", "127.0.0.1", 7709).unwrap()).build().unwrap();
    assert_eq!(config.pool_size(), 5);
    assert_eq!(config.async_connections(), 4);
    assert!(config.queue_capacity() > 0);
    assert_eq!(config.rate_limit().scope(), RateLimitScope::PerClient);
    assert!(!config.cache_policy().enabled());
}
```

- [ ] **Step 2: Run the test and verify unresolved configuration types**

Run: `cargo test -p magic-tdx-rs --test config`

Expected: compile failure.

- [ ] **Step 3: Implement checked endpoint and immutable request/response envelopes**

`Endpoint` stores non-empty name/host and non-zero port and redacts nothing needed for public market-data diagnostics. `RequestEnvelope` stores operation, request id, packet bytes, expected response family, attempt, and one absolute deadline. `ResponseEnvelope` stores validated header/body, endpoint, attempt, elapsed, fetched time, and trace id. Neither envelope substitutes source time. Define public configuration-only `RateLimitScope::{PerClient, PerConnection}`, `RateLimitConfig`, and `CachePolicy` here so `ClientConfig` is complete before the runtime limiter is implemented in Task 4.

- [ ] **Step 4: Implement `ClientConfig` and builder validation**

Include endpoints, connect/read/write/overall/pool/queue timeouts, pool size 5, async connections 4, bounded queue capacity, direct concurrency limit, retry budget, heartbeat interval, protocol `Limits`, `RateLimitConfig`, and `CachePolicy`. Reject zero resource limits, `overall_timeout` shorter than any required sub-timeout, rates above 200, empty endpoints, duplicate endpoints, and retry budgets whose worst-case attempts cannot fit the operation deadline.

- [ ] **Step 5: Verify builder behavior and commit configuration**

```bash
cargo test -p magic-tdx-rs --test config
cargo test -p magic-tdx-rs config
```

Expected: defaults and every invalid boundary pass their assertions.

```bash
git add crates/magic-tdx-rs/Cargo.toml crates/magic-tdx-rs/src/config.rs crates/magic-tdx-rs/src/transport crates/magic-tdx-rs/src/lib.rs crates/magic-tdx-rs/tests/config.rs
git diff --cached --check
git commit -m "feat(tdx): define bounded client configuration"
```

### Task 2: Build a reusable scripted loopback server

**Files:**
- Create: `crates/magic-tdx-rs/tests/support/mod.rs`
- Create: `crates/magic-tdx-rs/tests/support/loopback.rs`
- Create: `crates/magic-tdx-rs/tests/loopback.rs`

- [ ] **Step 1: Define script actions and observable events**

The test server accepts `Action::{ReadExact(usize), Send(Vec<u8>), SendChunks(Vec<Vec<u8>>), Delay(Duration), Disconnect, KeepOpen}` and records `Event::{Accepted, Read(Vec<u8>), Sent(usize), Disconnected}` per connection. Bind only `127.0.0.1:0`; expose the selected endpoint and a bounded event receiver.

- [ ] **Step 2: Test the harness independently**

Write a plain `TcpStream` test that scripts a split header/body response, verifies the exact request bytes, and joins the server thread with a timeout. A second test drops the client before `Send` and verifies the harness exits without hanging.

- [ ] **Step 3: Run and commit the loopback harness**

Run: `cargo test -p magic-tdx-rs --test loopback harness_`

Expected: both harness tests pass in under two seconds.

```bash
git add crates/magic-tdx-rs/tests/support crates/magic-tdx-rs/tests/loopback.rs
git diff --cached --check
git commit -m "test(tdx): add deterministic transport loopback"
```

### Task 3: Implement blocking TCP I/O and connection pooling

**Files:**
- Create: `crates/magic-tdx-rs/src/transport/blocking.rs`
- Create: `crates/magic-tdx-rs/src/transport/pool.rs`
- Modify: `crates/magic-tdx-rs/src/transport/mod.rs`
- Modify: `crates/magic-tdx-rs/tests/loopback.rs`

- [ ] **Step 1: Write failure-first blocking transport tests**

Add tests for split reads, header/body length mismatch, connect timeout, read timeout, server disconnect, pool exhaustion, deadline expiring while waiting, and protocol failure invalidating a connection. The invalidation test scripts a malformed response on connection 1 and a valid response on connection 2, then asserts connection 1 is never reused.

- [ ] **Step 2: Run tests and verify blocking transport is missing**

Run: `cargo test -p magic-tdx-rs --test loopback blocking_`

Expected: compile failure.

- [ ] **Step 3: Implement `BlockingConnection` with exact deadline propagation**

Use `TcpStream` connect/read/write timeouts capped by remaining overall time. Send the complete request, read the fixed response header exactly, reject declared sizes above limits before allocation, read the body exactly, validate/decompress it, and return contextual transport/protocol errors. A zero-byte read before completion is `Transport`, not an empty success.

- [ ] **Step 4: Implement a lock-safe bounded pool**

The pool maintains idle connections and total permits separately. Never hold the manager mutex during connect, read, or write. Borrow waits only to the earlier pool/operation deadline. A guard returns a connection only after `mark_reusable`; drop after transport/protocol desynchronization closes it and releases capacity. Expose an immutable `PoolSnapshot { configured, open, idle, borrowed, waiters, discarded }`.

- [ ] **Step 5: Stress pool state invariants**

Spawn 100 local threads against a pool of five and a server that returns fixed valid frames. Assert maximum simultaneous accepted connections is five, all permits return after success/error/panic-catching callers, and `borrowed + idle <= open <= configured` for every sampled snapshot.

- [ ] **Step 6: Run and commit blocking transport**

```bash
cargo test -p magic-tdx-rs --test loopback blocking_
cargo test -p magic-tdx-rs transport::pool
```

Expected: all pass without deadlock.

```bash
git add crates/magic-tdx-rs/src/transport/blocking.rs crates/magic-tdx-rs/src/transport/pool.rs crates/magic-tdx-rs/src/transport/mod.rs crates/magic-tdx-rs/tests/loopback.rs
git diff --cached --check
git commit -m "feat(tdx): add blocking transport pool"
```

### Task 4: Implement injectable adaptive rate limiting

**Files:**
- Create: `crates/magic-tdx-rs/src/rate_limit/mod.rs`
- Create: `crates/magic-tdx-rs/src/rate_limit/clock.rs`
- Create: `crates/magic-tdx-rs/src/rate_limit/limiter.rs`
- Create: `crates/magic-tdx-rs/tests/rate_limit.rs`
- Modify: `crates/magic-tdx-rs/src/lib.rs`

- [ ] **Step 1: Write deterministic fake-clock tests**

Test exact phase/rate mapping for pre-open, morning, lunch, afternoon, after-close, and invalid/unknown Shanghai time; exact rate ceiling 200 and rejection at 201; PerClient sharing across two connections; PerConnection independent budgets; cancellation/deadline while waiting; and disabled limiter producing no wait metadata.

- [ ] **Step 2: Run tests and verify limiter types are missing**

Run: `cargo test -p magic-tdx-rs --test rate_limit`

Expected: compile failure.

- [ ] **Step 3: Implement clock and phase contracts**

Define internal `Clock` with monotonic now, UTC wall time, and blocking/async delay hooks. Production uses system/Tokio time; tests use `ManualClock`. Convert wall time to Asia/Shanghai with explicit fixed rules and return `TradingPhase::Unknown` on ambiguity. Register and implement 15/30/60 RPS phase mapping; Unknown uses 15.

- [ ] **Step 4: Implement deadline-aware limiter state**

Use a token-bucket or virtual-schedule implementation whose capacity is bounded, observable, and tested. `acquire` returns wait duration and selected phase/rate; it checks the operation deadline before sleeping. Scope construction happens once per client or once per connection according to `RateLimitScope`.

- [ ] **Step 5: Run and commit the limiter**

```bash
cargo test -p magic-tdx-rs --test rate_limit
cargo test -p magic-tdx-rs rate_limit
```

Expected: no test sleeps in wall-clock time; all fake-clock cases pass.

```bash
git add crates/magic-tdx-rs/src/rate_limit crates/magic-tdx-rs/src/lib.rs crates/magic-tdx-rs/tests/rate_limit.rs docs/business_rules.md
git diff --cached --check
git commit -m "feat(tdx): add scoped adaptive rate limiting"
```

### Task 5: Implement `BlockingClient`

**Files:**
- Create: `crates/magic-tdx-rs/src/client/mod.rs`
- Create: `crates/magic-tdx-rs/src/client/blocking.rs`
- Create: `crates/magic-tdx-rs/tests/client_strategies.rs`
- Modify: `crates/magic-tdx-rs/src/lib.rs`

- [ ] **Step 1: Write BlockingClient behavior tests**

Construct through `BlockingClient::builder().endpoint(...).build()`. Test that one request borrows/reuses a healthy connection, a malformed response discards it, five concurrent operations progress with pool size five, the sixth times out at the configured pool deadline, limiter wait is included in overall timeout, and `snapshot()` exposes pool/limiter state without mutable internals.

- [ ] **Step 2: Run the focused tests and verify the client is absent**

Run: `cargo test -p magic-tdx-rs --test client_strategies blocking_`

Expected: compile failure.

- [ ] **Step 3: Implement builder and internal request execution**

`BlockingClient` owns immutable config, pool, limiter state, clock, and request-id generator. Its `pub(crate) execute(RequestEnvelope)` acquires rate permission, borrows, performs I/O, validates the response, records tracing fields, and marks the connection reusable only on a complete protocol-aligned response. Public surface exposes builder/config/snapshot; high-level market methods arrive in Phase 4.

- [ ] **Step 4: Verify and commit BlockingClient**

```bash
cargo test -p magic-tdx-rs --test client_strategies blocking_
cargo test -p magic-tdx-rs --test loopback blocking_
```

Expected: all pass.

```bash
git add crates/magic-tdx-rs/src/client/mod.rs crates/magic-tdx-rs/src/client/blocking.rs crates/magic-tdx-rs/src/lib.rs crates/magic-tdx-rs/tests/client_strategies.rs
git diff --cached --check
git commit -m "feat(tdx): add pooled blocking client"
```

### Task 6: Implement `DirectClient`

**Files:**
- Create: `crates/magic-tdx-rs/src/client/direct.rs`
- Modify: `crates/magic-tdx-rs/src/client/mod.rs`
- Modify: `crates/magic-tdx-rs/tests/client_strategies.rs`

- [ ] **Step 1: Write DirectClient isolation/concurrency tests**

Assert each request creates a distinct loopback connection, no request holds shared serialization state, a configured semaphore limits simultaneous connects, permit wait is covered by the overall deadline, one failed request cannot poison another, and dropping a request closes only its own socket.

- [ ] **Step 2: Run and verify the tests fail**

Run: `cargo test -p magic-tdx-rs --test client_strategies direct_`

Expected: compile failure.

- [ ] **Step 3: Implement direct execution**

`DirectClient` owns config, shared optional PerClient limiter, and a bounded concurrency semaphore. `execute` acquires deadline-aware permit and limiter, creates one `BlockingConnection`, performs one request/response, and closes it. Under PerConnection scope, construct a limiter for that connection; never add a pool or global request mutex.

- [ ] **Step 4: Verify 60-way concurrency and commit**

Run a loopback test with 60 callers and semaphore limit 60; assert the server observes overlapping connections. Then run:

```bash
cargo test -p magic-tdx-rs --test client_strategies direct_
git add crates/magic-tdx-rs/src/client/direct.rs crates/magic-tdx-rs/src/client/mod.rs crates/magic-tdx-rs/tests/client_strategies.rs
git diff --cached --check
git commit -m "feat(tdx): add isolated direct client"
```

### Task 7: Implement Tokio transport and `AsyncClient`

**Files:**
- Create: `crates/magic-tdx-rs/src/transport/asynchronous.rs`
- Create: `crates/magic-tdx-rs/src/client/asynchronous.rs`
- Modify: `crates/magic-tdx-rs/src/transport/mod.rs`
- Modify: `crates/magic-tdx-rs/src/client/mod.rs`
- Modify: `crates/magic-tdx-rs/tests/client_strategies.rs`

- [ ] **Step 1: Write async cancellation/backpressure tests**

Using `#[tokio::test(flavor = "multi_thread")]`, test four default connection tasks, exact round-robin assignment 0/1/2/3/0, bounded queue full behavior, operation timeout while queued, future cancellation before send and during read, connection task panic/exit propagation, heartbeat failure, malformed response replacement, and shutdown joining every task.

- [ ] **Step 2: Run and verify AsyncClient tests fail**

Run: `cargo test -p magic-tdx-rs --test client_strategies async_`

Expected: compile failure.

- [ ] **Step 3: Implement Tokio request/response I/O**

Mirror blocking framing with `AsyncReadExt`/`AsyncWriteExt` and `tokio::time::timeout_at`. Use one absolute Tokio deadline derived at request creation; validate sizes before allocation. Cancellation drops socket/task work for that request and completes its response channel with a typed cancellation/transport error when possible.

- [ ] **Step 4: Implement owned connection tasks and bounded dispatch**

Each connection task exclusively owns its socket, per-connection limiter when configured, heartbeat, and bounded `mpsc` receiver. The client chooses tasks with an atomic deterministic round-robin counter and sends a work item containing a `oneshot` response. Queue send and response wait are deadline-bounded. A task supervisor replaces failed tasks and exposes generation/failure counts; join errors become `Transport` with operation context.

- [ ] **Step 5: Implement graceful shutdown and snapshots**

`AsyncClient::shutdown().await` closes senders, cancels in-flight I/O, awaits every task, and reports join failures. Drop initiates cancellation without blocking. `snapshot().await` returns configured/live tasks, queue depth/capacity, round-robin cursor, restarts, and limiter state.

- [ ] **Step 6: Run and commit AsyncClient**

```bash
cargo test -p magic-tdx-rs --test client_strategies async_
cargo test -p magic-tdx-rs transport::asynchronous
```

Expected: all pass under Tokio's paused-time tests where timing is relevant.

```bash
git add crates/magic-tdx-rs/src/transport/asynchronous.rs crates/magic-tdx-rs/src/transport/mod.rs crates/magic-tdx-rs/src/client/asynchronous.rs crates/magic-tdx-rs/src/client/mod.rs crates/magic-tdx-rs/tests/client_strategies.rs
git diff --cached --check
git commit -m "feat(tdx): add bounded async client"
```

### Task 8: Implement `SmartClient` with one observable retry budget

**Files:**
- Create: `crates/magic-tdx-rs/src/client/smart.rs`
- Modify: `crates/magic-tdx-rs/src/client/mod.rs`
- Modify: `crates/magic-tdx-rs/tests/client_strategies.rs`

- [ ] **Step 1: Write endpoint health and exhaustion tests**

Script three endpoints and test initial deterministic order, latency/health ordering after probes, failure count, cooldown exclusion, re-entry after fake-clock advance, one total attempt count across selection/transport/empty retry, and aggregate error content when all endpoints fail. Test that a final empty response is `EmptyResponse` or `RetryExhausted`, never `Ok(Vec::new())`.

- [ ] **Step 2: Run and verify SmartClient is absent**

Run: `cargo test -p magic-tdx-rs --test client_strategies smart_`

Expected: compile failure.

- [ ] **Step 3: Implement observable endpoint state and deterministic selection**

Track successes, consecutive failures, last latency, last error kind, cooldown-until, and probe timestamp behind a narrow synchronized registry. Selection filters cooldown/blacklist, then orders by health score, latency, and configured index as a stable tiebreaker. Expose immutable redacted `ServerHealthSnapshot` values.

- [ ] **Step 4: Implement operation-scoped retry execution**

`SmartClient` wraps a configured blocking executor strategy and receives one `RetryBudget` per request. Every probe/failover/empty retry consumes from the same budget and respects the original absolute deadline. Collect endpoint/kind/attempt failures into `AggregateFailure`; do not expose `inner()` as an escape hatch or imply unsupported capabilities.

- [ ] **Step 5: Verify and commit SmartClient**

```bash
cargo test -p magic-tdx-rs --test client_strategies smart_
cargo test -p magic-tdx-rs client::smart
```

Expected: deterministic fake-clock cases pass.

```bash
git add crates/magic-tdx-rs/src/client/smart.rs crates/magic-tdx-rs/src/client/mod.rs crates/magic-tdx-rs/tests/client_strategies.rs docs/business_rules.md
git diff --cached --check
git commit -m "feat(tdx): add observable smart client"
```

### Task 9: Establish strategy benchmarks and close Phase 3

**Files:**
- Create: `crates/magic-tdx-rs/benches/loopback_clients.rs`
- Create: `docs/CLIENTS_AND_CONCURRENCY.md`
- Create: `docs/RATE_LIMITING.md`
- Modify: `crates/magic-tdx-rs/Cargo.toml`
- Modify: `.planning/2026-07-21-magic-tdx-rs/progress.md`

- [ ] **Step 1: Add Criterion strategy harnesses**

Benchmark one fixed valid quote response through Blocking, Direct, Async, and Smart with concurrency 1, 5, and 60. Run both compatible limiter configuration and explicitly disabled limiter. Record throughput, median/p95 where the harness supports it, max connections, queue depth, errors, and allocations/RSS collection hook. This phase compiles and smoke-runs the harness; Phase 5 compares against upstream.

- [ ] **Step 2: Document client-specific semantics**

`docs/CLIENTS_AND_CONCURRENCY.md` must explain ownership, defaults, backpressure, timeout composition, cancellation, recovery, snapshots, and why pooled modes are not expected to resemble Direct at 60 callers. `docs/RATE_LIMITING.md` must state BR-005, scope selection, phase schedule, clock behavior, disabled-benchmark interpretation, and the 200 RPS ceiling.

- [ ] **Step 3: Run the complete Phase 3 gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo test --workspace --doc --all-features
cargo doc --workspace --all-features --no-deps
cargo bench -p magic-tdx-rs --bench loopback_clients --no-run
bash tools/compliance/check.sh
```

Expected: every command exits `0`; tests prove no external network use.

- [ ] **Step 4: Record evidence, commit closeout, and stop**

```bash
git add crates/magic-tdx-rs/benches/loopback_clients.rs crates/magic-tdx-rs/Cargo.toml docs/CLIENTS_AND_CONCURRENCY.md docs/RATE_LIMITING.md .planning/2026-07-21-magic-tdx-rs/progress.md
git diff --cached --check
git commit -m "test(tdx): close client strategy phase"
```

Record exact loopback commands, max observed concurrency, queue/pool snapshots, timeout/cancellation evidence, and commit SHAs. Stop for review before Phase 4.
