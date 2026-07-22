# TDX capability matrix

The first delivery covers the complete pure-Rust capability surface of the
pinned `tdxrs` implementation. The following modules are present in the
`magic-tdx-rs` crate:

| Area | Implementation |
|---|---|
| Blocking pooled client | `TdxHqClient` |
| Direct client | `TdxDirectClient` |
| Tokio async client | `AsyncTdxHqClient` |
| Smart failover client | `TdxSmartClient` |
| Quotes and bars | `protocol::parsers`, `net::*` |
| Five-level order book | `OrderBooks` on `TdxHqClient` and `TdxSmartClient`; derived from quote bid/ask levels with source timestamps |
| Minute and transaction data | `protocol::parsers` |
| Finance and corporate actions | `protocol::finance_fields`, `protocol::adjuster` |
| Fund data | `fund` |
| Block data | `block` |
| F10/profile | `profile`, `TdxF10Client` |
| Local readers | `reader` |

The core contract also declares `MoneyFlows` and `Auctions`. TDX does not expose
auditable standardized feeds for those families, so their capabilities remain
explicitly `false`; callers receive an unsupported disposition rather than
fabricated zeros or empty successful batches.

Python/PyO3 bindings are excluded. Real-network validation is opt-in through
`examples/live_probe.rs`; deterministic validation is covered by the upstream
unit and parser suite.

`ProviderId::LocalTerminal` is reserved for an authorized, read-only local
terminal/SDK adapter. It must never read account, position, cash, or order
state, and it remains unimplemented until the terminal's official local API or
cache format is identified.
