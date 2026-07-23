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
rate limiting, heartbeat, disconnect, retry, normalized minute, Beijing market
mapping, and explicit unsupported-boundary tests.

## Live connectivity

On 2026-07-23 the read-only release `live_probe` completed against live TDX
services after SmartClient discarded an unavailable cached endpoint. It returned
stock and fund quotes, all 12 stock K-line categories, index bars, security
counts/list data, current and historical minute and transaction records,
real-time finance, corporate actions, three block families, fund data and F10.
The Beijing sample `920118` returned Quote price 16.91, five daily bars, two
five-level book sides, 120 current and 240 previous-session minute points, and
20 current trades. Beijing security metadata was explicitly unsupported because
market-2 security-list requests close at the live servers.
The financial archive stage downloaded 5,116,226 bytes from TDX's official data
host, parsed 5,526 records and extracted 45 named indicators for `600396`.
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

The probe now supports `quotes`, `bars`, `minute`, `trades`, and `mixed`.
Observed on the development machine on 2026-07-23 with `mixed`, which rotates
the four operation families for 华电辽能 `600396.SH`:

```text
requests=100 concurrency=8 successes=100 failures=0 records=3700
elapsed_seconds=1.770 requests_per_second=56.49
latency_us_p50=100077 latency_us_p95=219676 latency_us_max=251169
```

This is a short, bounded connectivity/load sample against an undocumented
public endpoint, not a vendor SLA or a safe sustained request rate. The probe
defaults deliberately remain small; operators must apply their own rate limit
and authorization policy.
