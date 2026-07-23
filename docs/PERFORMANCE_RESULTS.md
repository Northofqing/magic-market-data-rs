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
rate limiting, heartbeat, disconnect, and retry tests (219 TDX unit tests plus
adapter, capability, fuzz-smoke, golden and service integration tests).

## Live connectivity

On 2026-07-22 the read-only release `live_probe` completed against live TDX
services after SmartClient discarded an unavailable cached endpoint. It returned
stock and fund quotes, all 12 stock K-line categories, index bars, security
counts/list data, current and historical minute and transaction records,
real-time finance, corporate actions, three block families, fund data and F10.
The financial archive stage downloaded 5,116,020 bytes from TDX's official data
host, parsed 5,526 records and extracted 45 named indicators for `600519`.
The normalized transaction probe crossed real paging boundaries with
1,820/1,820 current and 2,001/2,001 historical records.

This is a connectivity and non-empty-result probe, not a latency or sustained
throughput benchmark. Live latency and throughput remain environment-dependent
and are not inferred from the parser microbenchmark above.

## MSRV verification

After a clean build, `RUSTUP_TOOLCHAIN=1.83.0 cargo check --workspace
--all-targets --offline` passes with the committed lockfile.

## Tencent HTTPS bounded load probe

Command:

```bash
cargo run -p magic-tencent-rs --example load_probe --release --offline
```

Observed on the development machine on 2026-07-23, requesting 华电辽能
`600396.SH` and 平安银行 `000001.SZ` in every request:

```text
requests=20 concurrency=4 successes=20 failures=0 records=40
elapsed_seconds=2.669 requests_per_second=7.49
latency_us_p50=152333 latency_us_p95=1772762 latency_us_max=2017417
```

This is a short, bounded connectivity/load sample against an undocumented
public endpoint, not a vendor SLA or a safe sustained request rate. The probe
defaults deliberately remain small; operators must apply their own rate limit
and authorization policy.
