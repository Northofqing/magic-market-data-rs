# Performance and connectivity results

## Deterministic parser probe

Command:

```bash
cargo run -p magic-tdx-rs --example parse_bench --offline
```

Observed on the development machine (debug profile, one million iterations):

```text
iterations=1000000 elapsed_ms=31.475 ns_per_op=31.48
```

This is a parser microbenchmark, not a network throughput claim.

## Local concurrency coverage

`cargo test --workspace --all-targets --offline` passes the imported TDX suite,
including async pool round-robin, concurrent channel operation, pool lifecycle,
rate limiting, heartbeat, disconnect, and retry tests (215 TDX unit tests plus
adapter, capability, fuzz-smoke, golden and service integration tests).

## Live connectivity

On 2026-07-22 the read-only release `live_probe` completed against live TDX
services after SmartClient discarded an unavailable cached endpoint. It returned
stock and fund quotes, all 12 stock K-line categories, index bars, security
counts/list data, current and historical minute and transaction records,
real-time finance, corporate actions, three block families, fund data and F10.
The financial archive stage downloaded 5,116,020 bytes from TDX's official data
host, parsed 5,526 records and extracted 45 named indicators for `600519`.

This is a connectivity and non-empty-result probe, not a latency or sustained
throughput benchmark. Live latency and throughput remain environment-dependent
and are not inferred from the parser microbenchmark above.

## MSRV verification

After a clean build, `RUSTUP_TOOLCHAIN=1.83.0 cargo check --workspace
--all-targets --offline` passes with the committed lockfile.
