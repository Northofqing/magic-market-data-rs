# magic-market-core

Provider-neutral checked market-data contracts.

The core distinguishes Shanghai, Shenzhen, and Beijing exchanges and exposes
source-evidenced contracts for quotes, bars, current/historical minute points,
trades, five-level books, money flow, auctions, and security metadata. Minute
points keep cumulative quantity and optional cumulative amount; metadata keeps
board, ST status, listing date, and the price-limit rule/version independently
optional. A provider may return fields it can prove without inventing the rest.

Construction and deserialization use the same validation boundary. Invalid
prices, quantities, identifiers, dates, evidence strings, OHLC ranges, and
contradictory quality reports are rejected instead of entering normalized
data. Validated requests expose read-only accessors so callers cannot bypass
their constructors with struct literals.

```rust
use magic_market_core::{
    AssetClass, DataBatch, Exchange, InstrumentId, Price, Provenance,
};

let instrument = InstrumentId::new(
    Exchange::Shanghai,
    "600396",
    AssetClass::Equity,
)?;
let price = Price::new(14.92)?;
let provenance = Provenance::new("tdx", "2026-07-22T10:00:01+08:00")?
    .with_source_at("2026-07-22T10:00:00+08:00")?
    .with_batch_id("tdx-quote-1")?;
let batch = DataBatch::strict(vec![(instrument, price)], provenance);

assert!(batch.quality().is_complete());
assert!(batch.quality().issues().is_empty());
# Ok::<(), magic_market_core::CoreError>(())
```
