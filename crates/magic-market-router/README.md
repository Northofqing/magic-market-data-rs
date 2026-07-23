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

The router does not merge Providers, cache responses, parse a universal
freshness age, add hidden retries or run a background service. Those operational
policies belong to the consuming application.

Run the real TDX-to-Tencent Quote route:

```bash
cargo run -p magic-market-router --example live_probe --release --locked --offline
```

TDX is tried first. Its normalized Quote lacks a verified source timestamp, so
the strict probe records a quality rejection and selects Tencent only when the
Tencent batch is complete and source-timestamped. A TDX connection failure is
also retained as a failed attempt rather than discarded.
