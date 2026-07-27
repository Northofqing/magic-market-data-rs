# magic-market-router

Provider-neutral, first-acceptable-batch failover for the normalized contracts
in `magic-market-core`.

The crate preserves an ordered trace of every attempted Provider and rejects
empty batches, mixed Provider IDs, missing provenance batch IDs and
record/provenance batch-ID mismatches under every policy. Callers may also
require complete batch quality and a source timestamp.

Provider errors are classified explicitly at registration:

```rust
use magic_market_core::ProviderId;
use magic_market_router::{
    quote_source, AcceptancePolicy, FailureKind, QuoteRouter, SourceError,
};
use magic_tencent_rs::{TencentClient, TencentError};
use std::sync::Arc;

let client = Arc::new(TencentClient::new()?);
let mut router = QuoteRouter::new(
    AcceptancePolicy::new().with_require_source_at(true),
);
router.register(quote_source(
    ProviderId::Tencent,
    client,
    |error| match error {
        TencentError::InvalidRequest(message) => {
            SourceError::stop(FailureKind::InvalidRequest, message)
        }
        TencentError::Unsupported(message) => {
            SourceError::try_next(FailureKind::Unsupported, message)
        }
        TencentError::Transport(message) => {
            SourceError::try_next(FailureKind::Transport, message)
        }
        other => SourceError::try_next(FailureKind::Protocol, other.to_string()),
    },
))?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The router does not merge Providers, cache responses, add hidden retries or run
a background service. Callers may opt into a strict `max_source_age` for
source-timestamped realtime families. That policy validates every record,
requires the batch timestamp to identify the oldest record, rejects ambiguous
date-only/timezone-free values and never substitutes `observed_at` for
`source_at`. Session selection remains an application decision.

`TargetPriceRouter` and `target_price_source` provide the provider-neutral
target-price route. The adapter admits exactly one complete consensus for the
requested instrument and date range, and checks the registered Provider,
provenance batch ID, aggregate/input evidence, canonical report ordering,
deduplication and derived observation/contributor boundaries before failover
selection. Provider-specific clients remain registered only at the application
boundary; the Router crate depends solely on Core contracts.

Run the real TDX-to-Tencent Quote route:

```bash
MAGIC_ROUTER_SESSION=continuous \
cargo run -p magic-market-router --example live_probe --release --locked --offline
```

TDX is tried first. Its normalized Quote lacks a verified source timestamp, so
the strict probe records a quality rejection and selects Tencent only when the
Tencent batch is complete and source-timestamped. A TDX connection failure is
also retained as a failed attempt rather than discarded.

The five-second example is only valid during continuous trading. Without
`MAGIC_ROUTER_SESSION=continuous`, it exits successfully with
`skipped_non_continuous_session` before contacting a Provider; pre-open, lunch,
post-close and replay consumers must choose a different policy explicitly.
The bounded 2026-07-27 continuous-session result and exact source age are
archived in
[`docs/evidence/2026-07-27-router-freshness.md`](../../docs/evidence/2026-07-27-router-freshness.md).
