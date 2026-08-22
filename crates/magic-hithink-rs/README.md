# magic-hithink-rs

Bounded Rust Provider for the official HITHINK Fuyao Financial API.

Production admission currently covers:

- explicit-range unadjusted A-share daily `HistoricalBars`;
- the PE/PB subset of `MarketStatistics`;
- explicit-date upper, lower and broken `LimitPools`;
- the current 24-hour `Popularity` list.

Set `HITHINK_FINANCE_API_KEY` in the service process environment. The key is
sent only in the `X-api-key` header and is redacted from Debug output. Missing,
expired or unauthorized keys fail explicitly without records or fallback.

```powershell
cargo run -p magic-hithink-rs --example live_probe --release --locked
cargo run -p magic-hithink-rs --example load_probe --release --locked
```

See [the Gate A and admission contract](../../docs/integrations/hithink-fuyao.md)
for exact endpoints, field mappings, exclusions and failure categories.
