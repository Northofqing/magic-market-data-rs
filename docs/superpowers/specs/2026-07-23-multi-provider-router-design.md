# Multi-provider failover router design

## 1. Goal and scope

Add a reusable Rust routing layer above `magic-market-core` so consumers can
register ordered providers per data family, accept the first trustworthy batch,
and retain an auditable record of every attempted source.

The first release covers all existing normalized families through one generic
primitive:

- realtime Quote;
- historical bars;
- current/historical minute data;
- current/historical trades;
- money flow;
- five-level order books;
- call auction;
- security metadata.

It does not implement cross-provider record merging, cache, persistence,
background scheduling, HTTP, database access or application-specific freshness
thresholds. Those policies remain outside the reusable router.

## 2. Approaches considered

### 2.1 Provider-neutral generic chain — selected

`magic-market-router` depends only on `magic-market-core`. A generic
`FailoverChain<Request, Record>` owns ordered object-safe sources. Small adapter
functions convert existing Core capability traits into sources while the caller
supplies the concrete provider-error classifier.

This keeps the router reusable, prevents dependency cycles, and allows future
providers without editing a closed enum.

### 2.2 Concrete TDX/Tencent/EMQuant enum — rejected

A single enum would be convenient initially but would make every new Provider a
router release and would pull all network/SDK dependencies into every consumer.

### 2.3 Routing inside `magic-market-core` — rejected

Core owns checked values and contracts. Provider ordering, fallback and
acceptance are operational policy and would make the contract crate depend on
runtime behavior.

## 3. Components and public contracts

### 3.1 `SourcedRecord`

Core adds a public evidence trait exposing only:

```rust
pub trait SourcedRecord {
    fn provider_id(&self) -> ProviderId;
    fn evidence_batch_id(&self) -> &str;
}
```

All eight normalized record types implement it through their existing checked
fields. The router uses it to reject a provider that returns another
ProviderId or a record batch ID different from batch provenance.

### 3.2 Source abstraction

```rust
pub trait RoutedSource<Request: ?Sized, Record>: Send + Sync {
    fn provider_id(&self) -> ProviderId;
    fn fetch(&self, request: &Request) -> Result<DataBatch<Record>, SourceError>;
}
```

`SourceFn` stores a bounded synchronous closure. Adapter constructors exist for
every Core provider trait, accepting `Arc<P>` and a
`Fn(P::Error) -> SourceError` classifier. Concrete provider crates do not depend
on the router.

### 3.3 Failure classification

`FailureKind` distinguishes invalid request, unsupported, transport, timeout,
rate limited, no data, protocol, quality, evidence contract and provider error.
`FailureAction` is either `Stop` or `TryNext`.

Invalid caller requests must be classified `Stop`. Unsupported endpoints,
network failures and no-data results can be `TryNext`. Provider-specific
protocol failures are classified explicitly by the registration site rather
than parsed from display text.

### 3.4 Acceptance policy

`AcceptancePolicy` independently configures:

- `require_complete`: reject batches with quality issues;
- `require_source_at`: reject batches without batch-level source time.

Every policy always rejects empty batches, ProviderId mismatch, missing
provenance batch ID and record/provenance batch-ID mismatch. The router does not
parse timestamps or invent a universal age threshold because data families and
trading phases require different freshness rules.

### 3.5 Route outcome and trace

The first accepted batch is returned unchanged inside `RouteOutcome<Record>`.
The outcome also exposes ordered `RouteAttempt` entries for failed, rejected and
selected providers. If no source succeeds, `RouterError` retains the same
attempt vector. No successful batch contains records from more than one source.

Registration rejects a duplicate ProviderId within one chain. An empty chain is
an explicit configuration error.

## 4. Data flow

1. The consumer creates a family-specific chain and acceptance policy.
2. It registers sources in priority order.
3. For each source, the chain invokes the same immutable request.
4. A terminal source error immediately returns `RouterError::Stopped`.
5. A retryable source error is recorded and advances to the next source.
6. A returned batch is checked for non-empty records, quality, source time and
   evidence consistency.
7. A rejected batch is recorded and advances to the next source.
8. The first accepted batch and complete attempt trace are returned.
9. Exhaustion returns every attempt; no empty success or stale cache is emitted.

## 5. Real-provider probe

`magic-market-router/examples/live_probe.rs` registers TDX before Tencent for
realtime Quote and requires complete quality plus batch source time.

TDX currently cannot prove its raw Quote time field, so its normalized batch is
rejected by policy rather than treated as a transport failure. Tencent is then
selected if its source-backed Quote is complete. The probe prints the selected
Provider, record evidence and every attempt, and exits nonzero unless the route
is genuinely accepted.

If TDX cannot connect, that failure remains in the trace and Tencent may still
succeed. If both fail, the probe fails; it never prints fixture data.

## 6. Tests

Deterministic tests cover:

- first-source success and no unnecessary later call;
- retryable failure followed by success;
- terminal invalid request preventing fallback;
- all-source exhaustion with ordered attempts;
- empty, incomplete and missing-source-time rejection;
- permissive partial-batch acceptance;
- duplicate ProviderId registration;
- record ProviderId mismatch;
- missing/mismatched provenance batch ID;
- adapter compilation and error classification for every Core data family.

The existing full workspace gates remain mandatory under Rust 1.83. The live
probe supplies network evidence but deterministic tests never require network.

## 7. Packaging and deployment

The release package adds `magic-router-live-probe` and continues using isolated
build directories plus SHA-256 manifests. Deployment documentation explains
that routing does not add retries inside providers, does not cache, and does not
make Tencent an SLA-backed primary source.

The current EMQuant authorization error remains visible. Operators may register
EMQuant only after the account has the required product entitlements.

## 8. Acceptance criteria

- The router crate has no dependency on TDX, Tencent or EMQuant.
- All normalized record families implement `SourcedRecord`.
- Invalid requests cannot fall through to another source.
- Every successful or failed route exposes ordered attempt evidence.
- Empty/mixed-provider/mismatched-batch results cannot be accepted.
- Deterministic tests and all release gates pass on Rust 1.83.
- The real TDX-to-Tencent Quote probe exits zero with a selected source.
- A five-probe package for the final Git SHA passes its SHA-256 verification.
