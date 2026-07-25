# magic-gov-rs

Credential-free, read-only Provider for the official China Government policy
library. It exposes the provider-neutral `PolicyDocuments` contract from
`magic-market-core` and accepts only canonical `www.gov.cn` documents.

```bash
cargo test -p magic-gov-rs --all-targets --locked --offline
cargo run -p magic-gov-rs --example live_probe --release --locked --offline
```

See [the integration contract](../../docs/integrations/gov-policy.md) for
request bounds, provenance and production limitations.
