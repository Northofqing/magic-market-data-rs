# Performance and connectivity results

## Evidence-driven release profile

On 2026-07-29, two clean revisions were measured in separate runner sessions
with:

```bash
bash tools/bench/release_profile.sh
```

The current runner requires a clean Git worktree and index, rejects inherited
Rust/Cargo build environment and automatic Cargo configuration, captures the
full revision, and builds both profiles from an isolated `git archive` source
snapshot with an isolated Cargo home. It builds separate default and candidate
target directories, warms each binary once, and then alternates five measured
runs per profile. It verifies the full porcelain state before evidence
collection and before exit. Inputs are fixed and offline. The candidate uses
`lto="thin"` and `codegen-units=1`; the default uses Cargo's release defaults.

Environment:

```text
rustc 1.95.0 (59807616e 2026-04-14)
cargo 1.95.0 (f2d3ce0bd 2026-03-21)
Darwin 25.5.0 x86_64
```

The comparison gate required identical checksums, at least 5% combined median
improvement, no workload regression above 5%, and binary growth no greater
than 20%.

The first clean session at revision
`8c8e9b5587ac48f4070e2524ea28fd4510836c77` produced:

| Workload | Iterations | Default median (ns) | Candidate median (ns) | Elapsed change | Checksum |
| --- | ---: | ---: | ---: | ---: | ---: |
| TDX bar/variable decode | 20,000 | 1,599,510,410 | 1,567,802,842 | -1.98% | 4,287,391,093,950,792,928 |
| JSON decode/normalization | 10,000 | 1,192,383,400 | 1,162,488,584 | -2.51% | 7,267,965,373,649,679,376 |
| zlib decompression | 5,000 | 709,281,762 | 730,768,802 | +3.03% | 440,610,000 |
| zlib compression/decompression roundtrip | 2,000 | 4,846,537,920 | 4,674,214,327 | -3.56% | 197,516,000 |

The geometric combined elapsed ratio was `0.987140`, a 1.29% improvement. The
example binary decreased from 663,992 to 631,792 bytes (-4.85%). This session
failed the predeclared 5% combined-improvement gate.

After the runner and raw-domain checks were hardened, a second clean session at
revision `d9555c6b06bcb27360a98a13765b8d0051ff575a` produced:

| Workload | Iterations | Default median (ns) | Candidate median (ns) | Elapsed change | Checksum |
| --- | ---: | ---: | ---: | ---: | ---: |
| TDX bar/variable decode | 20,000 | 1,645,003,943 | 1,550,589,876 | -5.74% | 4,287,391,093,950,792,928 |
| JSON decode/normalization | 10,000 | 1,258,431,039 | 1,157,620,254 | -8.01% | 7,267,965,373,649,679,376 |
| zlib decompression | 5,000 | 861,454,796 | 772,319,352 | -10.35% | 440,610,000 |
| zlib compression/decompression roundtrip | 2,000 | 4,969,610,920 | 4,731,810,894 | -4.79% | 197,516,000 |

The second session's geometric combined elapsed ratio was `0.927543`, a 7.25%
improvement, and its binary decreased from 664,120 to 631,792 bytes (-4.87%).
It passed the per-session policy. However, the sessions measured different
revisions: the later revision changed the TDX parser hot path and hardened the
runner, and its default binary size also differed. The results are therefore
not a same-revision repeatability experiment and cannot be compared to qualify
a workspace-wide optimization. The available evidence is insufficient, so the
repository fails closed and retains Cargo's default release profile.

Exact identities, all raw elapsed values, and both decisions are recorded in
[the release-profile evidence](evidence/2026-07-29-release-profile.md). These
are local deterministic microbenchmark results, not market-data latency or
network-throughput claims.

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

## Stable toolchain verification

After a clean build, `cargo check --workspace --all-targets --locked --offline`
passes with the repository's rolling stable toolchain. The project does not
declare a fixed MSRV.

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

The iWencai deterministic authentication/search suite passes on stable Rust.
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

This historical Tonghuashun run predates the Task 8 machine admission gate. It
must not be relabelled as current live admission. The current probe additionally
requires `status=admitted` or source-evidenced `status=verified_empty`; a
quality-incomplete empty-estimate pseudo-record now fails construction.

## SSE/SZSE/HKEX official mixed probes

Commit `904bd19` documented an upstream eight-operation historical baseline for
SSE/SZSE announcements, SZSE Quote/order book, SSE/SZSE dragon-tiger entries
and both HKEX northbound channels. That parent-commit result is not production
admission evidence for the merged tree and is intentionally not reported here
as a merged-tree pass.

Regenerate evidence from the exact candidate revision with:

```bash
cargo run -p magic-exchange-rs --example live_probe --release --locked --offline
MAGIC_EXCHANGE_LOAD_REQUESTS=8 MAGIC_EXCHANGE_LOAD_CONCURRENCY=1 \
  MAGIC_EXCHANGE_LOAD_PACING_MS=1000 \
  cargo run -p magic-exchange-rs --example load_probe --release --locked --offline
```

Archive the revision, compiler/Cargo versions, complete attempt output and
timestamps. High-level attempts may include multiple internally paced HTTP
requests, so their metrics are not HTTP throughput, an exchange SLA or
permission for sustained traffic.

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
