# magic-hithink-rs

Bounded Rust Provider for the official HITHINK Fuyao Financial API.

Production admission currently covers:

- explicit-range unadjusted A-share, standard-index and ETF daily `HistoricalBars`;
- the PE/PB subset of `MarketStatistics`;
- explicit-date upper, lower and broken `LimitPools`;
- the current 24-hour `Popularity` list;
- recent quarterly A-share `FinancialStatements` with per-report evidence;
- cash-dividend and bonus-share A-share `CorporateActions`;
- exact A-share/index/exchange-fund `SecurityMetadata` with unavailable fields left absent;
- current A-share `RealtimeQuotes` with numeric price/volume/turnover retained,
  while missing record `source_at` and name remain explicit as `Unavailable`;
- current A-share `CurrentAuctionObservations` for explicit `live` or `final`
  stage, including the provider-native signed `auction_unmatched` value.

The current-observation contract converts source lots to shares, treats a zero
price as an absent no-trade price, and preserves the response timestamp only as
`observed_at`. It does not claim a trading date, record `source_at`, unmatched
unit, or bid/ask direction. The separate complete Core `Auctions` mapping remains
an explicit unadmitted diagnostic because Fuyao does not return those required
fields. Use of another source or local time to fill them is prohibited.

Set `HITHINK_FINANCE_API_KEY` in the service process environment. The key is
sent only in the `X-api-key` header and is redacted from Debug output. Missing,
expired or unauthorized keys fail explicitly without records or fallback.

```powershell
cargo run -p magic-hithink-rs --example live_probe --release --locked
cargo run -p magic-hithink-rs --example load_probe --release --locked
```

See [the Gate A and admission contract](../../docs/integrations/hithink-fuyao.md)
for exact endpoints, field mappings, exclusions and failure categories.
