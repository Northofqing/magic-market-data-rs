# magic-hithink-rs

Bounded Rust Provider for the official HITHINK Fuyao Financial API.

Production admission currently covers:

- explicit-range unadjusted A-share, standard-index and ETF daily `HistoricalBars`;
- the PE/PB subset of `MarketStatistics`;
- explicit-date upper, lower and broken `LimitPools`;
- the current 24-hour `Popularity` list;
- recent quarterly A-share `FinancialStatements` with per-report evidence;
- cash-dividend and bonus-share A-share `CorporateActions`;
- exact A-share/index/exchange-fund `SecurityMetadata` with unavailable fields left absent.

The official current `stage=final` auction snapshot is also implemented as an
explicit diagnostic. It converts source lots to shares and preserves the
provider response timestamp as `observed_at`, but it is not production-admitted:
Fuyao does not return the exact trading date, record `source_at`, or directional
unmatched bid/ask quantities. Use of another source or local time to fill those
fields is prohibited.

Set `HITHINK_FINANCE_API_KEY` in the service process environment. The key is
sent only in the `X-api-key` header and is redacted from Debug output. Missing,
expired or unauthorized keys fail explicitly without records or fallback.

```powershell
cargo run -p magic-hithink-rs --example live_probe --release --locked
cargo run -p magic-hithink-rs --example load_probe --release --locked
```

See [the Gate A and admission contract](../../docs/integrations/hithink-fuyao.md)
for exact endpoints, field mappings, exclusions and failure categories.
