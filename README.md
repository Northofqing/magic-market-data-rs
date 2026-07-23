# magic-market-data-rs

Standalone Rust market-data workspace containing:

- `magic-market-core`: provider-neutral checked values and batch metadata.
- `magic-tdx-rs`: pure-Rust TDX protocol, readers, parsers, clients, funds,
  blocks, and F10/profile support extracted from upstream `tdxrs`.
- `magic-emquant-rs`: read-only Eastmoney/Choice provider using the separately
  built official-SDK snapshot bridge, without Rust `unsafe` or stored credentials.
- `magic-tencent-rs`: bounded HTTPS/GBK supplemental provider for verified
  Shanghai/Shenzhen A-share quotes and five-level order books.

The upstream implementation is retained under the MIT license. Python/PyO3
bindings are intentionally excluded. Network integration remains opt-in; the
deterministic test suite uses local fixtures and mocks.

## Verification

```bash
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
```

Deployment layout, platform constraints, health checks, rollback, and release
packaging are documented in [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md). Provider
details are in [TDX capabilities](docs/TDX_CAPABILITIES.md),
[Eastmoney EMQuant](docs/integrations/eastmoney-emquant.md), and
[Tencent web quotes](docs/integrations/tencent-web.md).
