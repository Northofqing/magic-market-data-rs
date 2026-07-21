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
| Minute and transaction data | `protocol::parsers` |
| Finance and corporate actions | `protocol::finance_fields`, `protocol::adjuster` |
| Fund data | `fund` |
| Block data | `block` |
| F10/profile | `profile`, `TdxF10Client` |
| Local readers | `reader` |

Python/PyO3 bindings are excluded. Real-network validation is opt-in through
`examples/live_probe.rs`; deterministic validation is covered by the upstream
unit and parser suite.
