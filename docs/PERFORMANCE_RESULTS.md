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

## CLS and Baidu bounded public-web probes

On 2026-07-23 the current CLS release probe returned five real global
telegraph/news records from the signed `www.cls.cn` endpoint and completed with
`live_probe_status=passed`. The intentionally serial two-request load run
observed:

```text
requests=2 concurrency=1 min_interval_ms=1000 successes=2 failures=0 records=20
elapsed_seconds=1.126 requests_per_second=1.777
latency_us_p50=252611 latency_us_p95=873075 latency_us_p99=873075
latency_us_max=873075
load_probe_status=passed
```

The current Baidu release probe returned five real unadjusted daily bars
for 华电辽能 `600396.SH`, including source MA5/MA10/MA20, and completed with
`live_probe_status=passed`. Its serial two-request load run fetched 20 bars per
request:

```text
requests=2 concurrency=1 min_interval_ms=1000 successes=2 failures=0 records=40
elapsed_seconds=1.268 requests_per_second=1.577
latency_us_p50=294120 latency_us_p95=973862 latency_us_p99=973862
latency_us_max=973862
load_probe_status=passed
```

Both clients share their serial gate across clones, enforce at least one second
between request starts, hold the gate through the complete response read, and
cap a load run at three requests. The probes report every error and fail the
process if any request fails. The numbers demonstrate current connectivity and
parser behavior, not endpoint SLA or permission for sustained traffic.

## iWencai authenticated probe status

The iWencai deterministic authentication/search suite passes on Rust 1.83.
Without `MAGIC_IWENCAI_API_KEY`, the real probe exits non-zero with the typed
`Authentication` error as designed; it does not import a browser session or
print simulated documents. `semantic_search` consequently remains false in the
advertised capability set even though the typed method and fixture parser are
implemented. Real throughput remains unmeasured until an authorized API key is
configured.

## CNInfo and Tonghuashun bounded public-web probes

The 2026-07-23 CNInfo live probe returned three 华电辽能 announcements and three
比亚迪 investor Q&A records. Canonical announcement detail URLs were checked
independently and returned HTTP 200. The serial announcement load probe
completed:

```text
requests=3 concurrency=1 successes=3 failures=0
elapsed_ms=3158 throughput_requests_per_second=0.9498
latency_min_ms=772 latency_p50_ms=795 latency_p95_ms=1381 latency_max_ms=1381
minimum_attempt_start_gap_ms=1004
```

The Tonghuashun live probe returned non-empty consensus, strong-stock reasons,
upper-limit-pool records and popularity records. Its serial popularity load
probe completed:

```text
requests=3 concurrency=1 successes=3 failures=0
elapsed_ms=2099 throughput_requests_per_second=1.4288
latency_min_ms=93 latency_p50_ms=106 latency_p95_ms=167 latency_max_ms=167
minimum_attempt_start_gap_ms=1002
```

Both providers hold a shared request gate through the complete response read
and enforce at least one second between request starts. These runs verify
current connectivity, non-empty parsing and pacing only; they are not endpoint
SLAs or permission for sustained traffic.

## SSE/SZSE official announcement probes

On 2026-07-23 the combined release live probe returned three real official
announcements for 华电辽能 `600396.SH` from SSE and three for 五粮液
`000858.SZ` from SZSE. Every record printed the source security, publication
date, canonical/PDF URL and SSE/SZSE evidence, and the process ended with
`live_probe_status=passed`.

The final alternating, serial load run recorded:

```text
attempts=4 successes=4 failures=0
measurement_elapsed_ms_excluding_output=4304
operation_elapsed_total_ms=2458 pacing_wait_total_ms=1845
attempt_throughput_per_second=0.9294
attempt_latency_p50_ms=1082
attempt_latency_p95_ms=1214
attempt_latency_p99_ms=1214
attempt_latency_max_ms=1214
minimum_attempt_start_gap_ms=1003
load_probe_status=passed
```

These are high-level announcement-attempt metrics sampled before batch output,
not HTTP request throughput; pagination can issue multiple requests inside one attempt. SZSE's
sampled detail page returned HTTP 200 and its sampled PDF returned
`application/pdf`. SSE supplied official PDF URLs, but a sampled HEAD request
was answered with CDN bot HTML, so no SSE PDF-download success is claimed.

## Eastmoney public-web probe status

The current live probe obtained real instrument/industry reports, industry,
concept and region board flows, dragon-tiger entries and seats, margin data,
block trades, holder counts, lockups, dividends, all four limit-pool families,
and popularity with separate quote evidence.

Both current fund-flow hosts closed the development network connection before
an HTTP response; an independent reference request reproduced the same empty
reply. Deterministic fund-flow fixtures and mapping tests pass, but the
capability remains unadvertised and this document does not claim real fund-flow
acceptance. The live probe records those calls as expected-failure diagnostics
and passes only after every advertised family has returned a strict, non-empty
batch. The final three-operation serial advertised-capability load run was:

```text
high_level_attempts=3 concurrency=1
admitted_successful_attempts=3 admitted_failed_attempts=0
diagnostic_complete_unadmitted_attempts=0 diagnostic_failed_attempts=0
total_elapsed_ms=2045 attempts_per_second=1.4664
attempt_latency_p50_ms=40 attempt_latency_p95_ms=45
attempt_latency_p99_ms=45 attempt_latency_max_ms=45 limiter_wait_total_ms=1930
minimum_attempt_start_gap_ms=1002
load_probe_status=passed
```

Reports, board flow and upper-limit pool all returned real non-empty batches.
An operator can still request the fund-flow diagnostic explicitly; that
diagnostic is expected to fail until a live host is independently accepted.
Keyword-news search is also an unadmitted diagnostic because the response does
not carry a structured source instrument identity; it is not counted as
instrument-news acceptance.
