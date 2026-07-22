# Contributing

Run the following before submitting a change:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
bash tools/compliance/check.sh
bash tools/docs/check_links.sh
```

Network diagnostics are read-only and must be reported separately from local
deterministic tests. Preserve the upstream MIT notice when changing imported
TDX modules.
