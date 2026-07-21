# magic-market-data-rs

Standalone Rust market-data workspace containing:

- `magic-market-core`: provider-neutral checked values and batch metadata.
- `magic-tdx-rs`: pure-Rust TDX protocol, readers, parsers, clients, funds,
  blocks, and F10/profile support extracted from upstream `tdxrs`.

The upstream implementation is retained under the MIT license. Python/PyO3
bindings are intentionally excluded. Network integration remains opt-in; the
deterministic test suite uses local fixtures and mocks.

## Verification

```bash
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
```
