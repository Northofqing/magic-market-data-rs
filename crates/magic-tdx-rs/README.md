# magic-tdx-rs

Pure-Rust TDX source driver.

## Stable service facades

The crate exposes typed service entry points in addition to the low-level
clients. All network calls are read-only and return the upstream error type.

| Facade | Coverage |
| --- | --- |
| `TdxService` | Smart failover K-lines, quotes, chunked quotes, securities, minute/trade history, finance and XDXR |
| `AsyncTdxService` | Async-pool equivalents, including atomic security-list pagination |
| `BlockService` | Industry, concept, index blocks, block K-lines and quotes |
| `FundService` | Fund/ETF list, bars, quotes, finance and XDXR |
| `FinanceService` | Realtime finance, report files/records, 45 named indicators |
| `ProfileService` | F10 categories, named sections, all sections and complete payloads |

Construct a facade without connecting; configure/connect its underlying client
explicitly before making live requests. For a read-only smoke test covering all
protocol families, run:

```text
cargo run -p magic-tdx-rs --example live_probe --release
```
