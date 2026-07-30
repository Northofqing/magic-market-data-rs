# magic-market-composition

This crate is the explicit concrete-provider composition boundary for
`magic-market-data-rs`.

`magic-market-core` owns provider-neutral contracts and
`magic-market-router` owns provider-neutral failover. A source that requires an
unforgeable concrete admission is bound here, where both the provider and
Router dependencies are allowed.

The first binding is `EastmoneyProviderTopNRankingRouter`. Its zero-argument
constructor creates the production `EastmoneyClient` internally and exposes
neither client injection nor generic registration. It therefore cannot be
populated by a downstream-local transport or wrapper claiming Eastmoney
metadata. Every result is still revalidated by Core, and the current
Asia/Shanghai date is recomputed for every route.

## Production Provider Top-N probe

The bounded production probe constructs only
`EastmoneyProviderTopNRankingRouter::new()`. It exposes no client or transport
injection and always routes `VolumeRatio` and `MainNetInflow` exactly once
each. It continues after the first metric fails, prints per-metric provider,
source, observation/source time, record count and provider-declared total, then
exits non-zero when the final failure count is not zero.

`MAGIC_COMPOSITION_TOPN_DATE` defaults to the current Asia/Shanghai calendar
date. The kind must remain `all`; the limit must be positive and no greater
than the proved one-response cap of 100.

```bash
MAGIC_COMPOSITION_TOPN_DATE=<current-Asia/Shanghai-date> \
MAGIC_COMPOSITION_TOPN_LIMIT=20 \
MAGIC_COMPOSITION_TOPN_KIND=all \
cargo run -p magic-market-composition \
  --example provider_top_n_live_probe --locked
```

This command performs real network I/O. Deterministic validation must use
`cargo test -p magic-market-composition --all-targets --locked --offline`
instead.
