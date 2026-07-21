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
rate limiting, heartbeat, disconnect, and retry tests (208 TDX tests).

## Live connectivity

The read-only `live_probe` example has successfully returned one quote and five
K-line records after SmartClient discarded an unavailable cached endpoint and
failed over to a working TDX server. Live latency and sustained throughput are
environment-dependent and are not claimed by the microbenchmark above.
