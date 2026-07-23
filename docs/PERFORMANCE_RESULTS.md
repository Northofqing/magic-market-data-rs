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

The probe now supports `quotes`, `bars`, `minute`, `trades`, `statistics`, and
`mixed`. The following historical sample was observed on the development
machine on 2026-07-23 before statistics joined the rotation; that version of
`mixed` rotated the four base operation families for 华电辽能 `600396.SH`:

```text
requests=100 concurrency=8 successes=100 failures=0 records=3700
elapsed_seconds=1.770 requests_per_second=56.49
latency_us_p50=100077 latency_us_p95=219676 latency_us_max=251169
```

After `MarketStatisticsProvider` was added, a dedicated current-code statistics
sample for 华电辽能、上证指数 and 上证 50 ETF completed:

```text
operation=statistics requests=12 concurrency=3 successes=12 failures=0 records=36
requests_per_second=28.76
latency_us_p50=66801 latency_us_p95=181955 latency_us_max=192500
```

This is a short, bounded connectivity/load sample against an undocumented
public endpoint, not a vendor SLA or a safe sustained request rate. The probe
defaults deliberately remain small; operators must apply their own rate limit
and authorization policy.

## Sina HTTPS bounded load probes

The current probe supports `quotes`, `bars`, `minute`, `financial`, `options`,
and `mixed`, with hard limits of 40 requests and four workers. The base-data
mixed sample on 2026-07-23 completed:

```text
requests=20 concurrency=4 successes=20 failures=0 records=1477
requests_per_second=11.69
latency_us_p50=207786 latency_us_p95=645489 latency_us_max=788549
```

Dedicated current-code samples for the newly added sources completed. The
option run covered 510050; 510300, 588000 and 510500 are implemented but have
not yet passed separate live runs:

```text
operation=financial requests=6 concurrency=2 successes=6 failures=0 records=48
requests_per_second=18.19
latency_us_p50=50705 latency_us_p95=210571 latency_us_max=213895

operation=options requests=6 concurrency=2 successes=6 failures=0 records=24
requests_per_second=22.30
latency_us_p50=62005 latency_us_p95=131468 latency_us_max=144344
```

The option sample used real contracts discovered immediately before the run.
These are short connectivity/load observations, not published endpoint capacity
or permission to sustain the measured rate.

After the fixed-contract fallback was removed, the current load probe was
rerun without `MAGIC_SINA_OPTION_CONTRACTS`. It discovered two current 510050
contracts once before starting the timed worker and completed:

```text
option_load_contracts source=discovery underlying=510050 count=2
operation=options requests=2 concurrency=1 successes=2 failures=0 records=8
requests_per_second=18.32
latency_us_p50=54445 latency_us_p95=54445 latency_us_max=54551
load_probe_status=passed
```
