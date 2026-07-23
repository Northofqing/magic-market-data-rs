# Multi-provider Router Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a provider-neutral, evidence-preserving failover router, a real TDX-to-Tencent routing probe, and a verified five-probe release package.

**Architecture:** `magic-market-core` exposes a common sourced-record evidence trait. A new `magic-market-router` crate depends only on Core and implements generic object-safe sources, explicit provider-error classification, acceptance policy, ordered attempt traces, and first-acceptable-batch routing. Concrete providers are wired only by generic adapter functions and the live example.

**Tech Stack:** Rust 2021, MSRV 1.83, `magic-market-core`, `thiserror`, Cargo workspace tests, release shell scripts.

---

### Task 1: Add common record evidence

**Files:**
- Modify: `crates/magic-market-core/src/provider.rs`
- Modify: `crates/magic-market-core/src/lib.rs`
- Create: `crates/magic-market-core/tests/sourced_record.rs`

- [ ] **Step 1: Write the failing public-contract test**

```rust
use magic_market_core::{ProviderId, Quote, SourcedRecord};

fn assert_sourced<T: SourcedRecord>() {}

#[test]
fn normalized_quote_exposes_common_evidence() {
    assert_sourced::<Quote>();
    let getter: fn(&Quote) -> ProviderId = SourcedRecord::provider_id;
    let batch: fn(&Quote) -> &str = SourcedRecord::evidence_batch_id;
    let _ = (getter, batch);
}
```

- [ ] **Step 2: Verify the type is absent**

Run:

```bash
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-market-core --test sourced_record --locked --offline
```

Expected: compile failure for missing `SourcedRecord`.

- [ ] **Step 3: Implement the trait for every normalized record**

Add this contract and explicit implementations for `Quote`, `Bar`,
`MinutePoint`, `Trade`, `MoneyFlow`, `OrderBook`, `AuctionSnapshot` and
`SecurityMetadata`:

```rust
pub trait SourcedRecord {
    fn provider_id(&self) -> ProviderId;
    fn evidence_batch_id(&self) -> &str;
}
```

Each method delegates to the type's existing checked `provider()` and
`batch_id()` accessors. Re-export the trait from Core.

- [ ] **Step 4: Verify Core**

Run:

```bash
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-market-core --all-targets --locked --offline
```

Expected: all Core tests pass.

### Task 2: Create router source and error contracts

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/magic-market-router/Cargo.toml`
- Create: `crates/magic-market-router/src/lib.rs`
- Create: `crates/magic-market-router/src/error.rs`
- Create: `crates/magic-market-router/src/source.rs`
- Create: `crates/magic-market-router/tests/source.rs`

- [ ] **Step 1: Write failing source-contract tests**

Tests construct a `SourceFn<[InstrumentId], Quote>` and assert its ProviderId,
successful fetch, and explicit `SourceError` kind/action:

```rust
let source = SourceFn::new(ProviderId::Custom, move |_| Ok(batch.clone()));
assert_eq!(source.provider_id(), ProviderId::Custom);
assert_eq!(source.fetch(&[instrument()]).unwrap().records().len(), 1);

let error = SourceError::new(
    FailureKind::InvalidRequest,
    FailureAction::Stop,
    "duplicate instrument",
);
assert_eq!(error.action(), FailureAction::Stop);
```

- [ ] **Step 2: Verify the new crate is absent**

Run:

```bash
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-market-router --test source --locked --offline
```

Expected: Cargo reports that package `magic-market-router` does not exist.

- [ ] **Step 3: Add the crate and contracts**

Implement:

```rust
pub enum FailureKind {
    InvalidRequest,
    Unsupported,
    Transport,
    Timeout,
    RateLimited,
    NoData,
    Protocol,
    Quality,
    Evidence,
    Provider,
}

pub enum FailureAction {
    Stop,
    TryNext,
}

pub struct SourceError {
    kind: FailureKind,
    action: FailureAction,
    message: String,
}

pub trait RoutedSource<Request: ?Sized, Record>: Send + Sync {
    fn provider_id(&self) -> ProviderId;
    fn fetch(&self, request: &Request) -> Result<DataBatch<Record>, SourceError>;
}
```

`SourceFn` owns an `Arc<dyn Fn(&Request) -> Result<DataBatch<Record>,
SourceError> + Send + Sync>`. Its constructor rejects no runtime state and its
debug representation omits the closure.

- [ ] **Step 4: Verify source contracts**

Run:

```bash
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-market-router --test source --locked --offline
```

Expected: source tests pass.

### Task 3: Implement acceptance and failover state machine

**Files:**
- Create: `crates/magic-market-router/src/router.rs`
- Create: `crates/magic-market-router/tests/router.rs`
- Modify: `crates/magic-market-router/src/lib.rs`

- [ ] **Step 1: Write failing state-machine tests**

Add isolated fake sources proving:

```rust
let outcome = chain.route(&[instrument()]).unwrap();
assert_eq!(outcome.selected_provider(), ProviderId::Tencent);
assert_eq!(outcome.attempts().len(), 2);
assert!(matches!(
    outcome.attempts()[0].status(),
    AttemptStatus::Failed { kind: FailureKind::Transport, .. }
));
assert!(matches!(outcome.attempts()[1].status(), AttemptStatus::Selected));
```

Separate tests cover terminal stop, exhaustion, empty batch, incomplete quality,
missing source time, duplicate registration, ProviderId mismatch, missing
provenance batch ID and record/provenance batch-ID mismatch.

- [ ] **Step 2: Verify tests fail on missing router types**

Run:

```bash
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-market-router --test router --locked --offline
```

Expected: compile failure for `FailoverChain`, `AcceptancePolicy` and trace types.

- [ ] **Step 3: Implement acceptance and routing**

The public API is:

```rust
pub struct AcceptancePolicy {
    require_complete: bool,
    require_source_at: bool,
}

pub struct FailoverChain<Request: ?Sized, Record> {
    policy: AcceptancePolicy,
    sources: Vec<Box<dyn RoutedSource<Request, Record>>>,
}

impl<Request: ?Sized, Record: SourcedRecord>
    FailoverChain<Request, Record>
{
    pub fn register<S>(&mut self, source: S) -> Result<&mut Self, RouterError>
    where
        S: RoutedSource<Request, Record> + 'static;

    pub fn route(&self, request: &Request)
        -> Result<RouteOutcome<Record>, RouterError>;
}
```

The route validates evidence before selection, records each attempt in order,
stops on `FailureAction::Stop`, and returns `Exhausted` only after every
registered source is tried. `RouteOutcome` owns the unchanged `DataBatch` and
the trace.

- [ ] **Step 4: Verify the full router state machine**

Run:

```bash
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-market-router --all-targets --locked --offline
```

Expected: all router tests pass.

### Task 4: Adapt every Core provider family

**Files:**
- Create: `crates/magic-market-router/src/adapters.rs`
- Create: `crates/magic-market-router/tests/adapters.rs`
- Modify: `crates/magic-market-router/src/lib.rs`

- [ ] **Step 1: Write compile-and-behavior tests**

Use small fake providers implementing each Core trait and construct:

```rust
quote_source(provider, client.clone(), classify);
bars_source(provider, client.clone(), classify);
minute_source(provider, client.clone(), classify);
trades_source(provider, client.clone(), classify);
money_flow_source(provider, client.clone(), classify);
order_book_source(provider, client.clone(), classify);
auction_source(provider, client.clone(), classify);
security_metadata_source(provider, client, classify);
```

Each source must invoke the corresponding trait exactly once and preserve its
returned `DataBatch`.

- [ ] **Step 2: Verify adapter constructors are absent**

Run:

```bash
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-market-router --test adapters --locked --offline
```

Expected: compile failure for missing adapter functions.

- [ ] **Step 3: Implement generic adapters and family aliases**

Each constructor accepts `ProviderId`, `Arc<P>`, and a thread-safe
`Fn(P::Error) -> SourceError`. Add aliases:

```rust
pub type QuoteRouter = FailoverChain<[InstrumentId], Quote>;
pub type BarsRouter = FailoverChain<BarsRequest, Bar>;
pub type MinuteRouter = FailoverChain<MinuteDataRequest, MinutePoint>;
pub type TradesRouter = FailoverChain<TradesRequest, Trade>;
pub type MoneyFlowRouter = FailoverChain<[InstrumentId], MoneyFlow>;
pub type OrderBookRouter = FailoverChain<[InstrumentId], OrderBook>;
pub type AuctionRouter = FailoverChain<[InstrumentId], AuctionSnapshot>;
pub type SecurityMetadataRouter =
    FailoverChain<[InstrumentId], SecurityMetadata>;
```

- [ ] **Step 4: Verify adapters and public API**

Run:

```bash
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-market-router --all-targets --locked --offline
```

Expected: all adapter and router tests pass.

### Task 5: Add real TDX-to-Tencent routing probe

**Files:**
- Modify: `crates/magic-market-router/Cargo.toml`
- Create: `crates/magic-market-router/examples/live_probe.rs`
- Create: `crates/magic-market-router/README.md`

- [ ] **Step 1: Add provider dev-dependencies and the strict probe**

The example builds a `QuoteRouter` with:

```rust
let policy = AcceptancePolicy::new()
    .with_require_complete(true)
    .with_require_source_at(true);
```

It registers TDX then Tencent, explicitly maps `TdxError` and `TencentError`,
prints every `RouteAttempt`, selected provider, Quote values and evidence, and
exits nonzero on route failure or missing records. A failed initial TDX
connection becomes a retryable TDX source; it is not discarded.

- [ ] **Step 2: Verify the example compiles**

Run:

```bash
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-market-router --all-targets --locked --offline
```

Expected: example and tests compile successfully.

- [ ] **Step 3: Run the real route**

Run:

```bash
RUSTUP_TOOLCHAIN=stable cargo run -p magic-market-router --example live_probe --release --locked --offline
```

Expected: a selected provider, non-empty Quote batch, ordered attempts and
`router_live_probe_status=passed`.

### Task 6: Integrate release, docs and gates

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/DEPLOYMENT.md`
- Create: `docs/MULTI_PROVIDER_ROUTING.md`
- Modify: `tools/release/package.sh`
- Modify: `tools/compliance/check.sh`
- Modify: `.planning/2026-07-23-multi-provider-router/task_plan.md`
- Modify: `.planning/2026-07-23-multi-provider-router/progress.md`

- [ ] **Step 1: Update workspace/release assertions**

Add `magic-market-router` to the exact workspace member list and compliance
assertion. Package the router example as `magic-router-live-probe` and document
the five-bin artifact layout and health-check order.

- [ ] **Step 2: Document operational boundaries**

Document ordered source registration, error classification, acceptance policy,
trace inspection, no merge/cache behavior, TDX source-time rejection and
EMQuant entitlement gating.

- [ ] **Step 3: Run final release preflight**

Run:

```bash
bash tools/release/preflight.sh
```

Expected: `release preflight: passed`.

- [ ] **Step 4: Self-review the complete diff**

Run:

```bash
git diff --check
git status --short
```

Review every changed file for secret leakage, silent fallback, wrong ProviderId,
stale documentation and accidental inclusion of the user's untracked document.

- [ ] **Step 5: Commit the implementation**

Stage only router/Core/release/docs/planning files and commit:

```bash
git commit -m "feat: add evidence-preserving provider router"
```

- [ ] **Step 6: Build and verify the final release artifact**

Run:

```bash
bash tools/release/package.sh
cd target/dist/FINAL_GIT_SHA
shasum -a 256 -c SHA256SUMS
```

Expected: five binaries and every checksum passes.

- [ ] **Step 7: Push and verify**

Run:

```bash
git push
git rev-parse HEAD
git rev-parse '@{u}'
```

Expected: local and upstream SHAs are identical.
