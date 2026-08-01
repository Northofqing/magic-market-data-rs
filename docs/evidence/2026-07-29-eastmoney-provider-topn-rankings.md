# Eastmoney Provider Top-N Ranking Admission Evidence

> Historical evidence note: this document records the original same-date
> admission run. Observation-date semantics were subsequently amended by
> `docs/superpowers/specs/2026-08-01-eastmoney-provider-topn-settled-capture-design.md`;
> exact per-row `f297` binding remains unchanged.

**Date:** 2026-07-29 Asia/Shanghai
**Rules:** BR-009, BR-010, BR-011, BR-021, BR-033, BR-034

## Contract boundary

This evidence covers only one Eastmoney `clist/get` response page for each
requested metric after 15:35 on the current China date. It does not admit the
existing complete-universe `MarketRankings` contract and does not prove market
breadth, complete coverage, an intraday ranking timestamp or a complete cutoff
tie set.

Each admitted record retains the provider response order as a local
`source_order_ordinal`, exact A-share identity and source name, value/unit,
per-security `f297` as `latest_trading_date`, the request-bound filter identity,
`data.total` as `provider_declared_total`, exact inspected row count, and
evidence with no `source_at`.

## Deterministic validation

Current targeted evidence:

- Core Provider Top-N contract tests passed.
- Eastmoney adapter tests passed, including collision separation across
  metrics, normalized response content, mixed source-date rejection and
  unsupported-unit rejection.
- Composition library, public-boundary and production-probe helper tests
  passed.
- The final workspace/all-target/all-feature test, Clippy and release
  preflight passed after the real production-composition probe.
- The existing full-market ranking types and capabilities were unchanged and
  remained unadmitted.

The suites reject wrong/missing `f297`, partial cardinality, missing metric,
name or identity, duplicate securities, wrong response order, negative volume
ratio, pre-15:35 capture, wrong offset/date and cross-midnight completion.

## Same-day diagnostic

The production Rust client ran after 15:35 Asia/Shanghai:

```bash
MAGIC_EASTMONEY_LIVE_OPERATION=provider-topn-rankings \
MAGIC_EASTMONEY_TOPN_DATE=2026-07-29 \
MAGIC_EASTMONEY_RANKING_KIND=all \
cargo run -p magic-eastmoney-rs --example live_probe --locked
```

Both independently checked metrics passed through the formal provider trait.
The probe now records request start and response-complete observation time so
the 15:35 same-date gate is directly auditable:

```text
=== provider_top_n_rankings.VolumeRatio ===
admitted=true
acquisition_started_at=2026-07-29T22:25:22.507854+08:00
status=admitted
records=20
batch_observed_at=2026-07-29T22:25:25+08:00
batch_source_at=None
first_record_observed_at=2026-07-29T22:25:25+08:00
provider_declared_total=5542
inspected_row_count=20
latest_trading_date=2026-07-29
first=002889.SZ 东方嘉盛 value=9.03
last=688496.SH *ST清越 value=4.01

=== provider_top_n_rankings.MainNetInflow ===
admitted=true
acquisition_started_at=2026-07-29T22:25:25.629507+08:00
status=admitted
records=20
batch_observed_at=2026-07-29T22:25:29+08:00
batch_source_at=None
first_record_observed_at=2026-07-29T22:25:29+08:00
provider_declared_total=5542
inspected_row_count=20
latest_trading_date=2026-07-29
first=688825.SH C长鑫 value=3453920512
last=002498.SZ 汉缆股份 value=364320606

failures=0
live_probe_status=admitted
```

All 40 selected records had exact identities, non-empty source names, complete
requested metrics and `latest_trading_date=2026-07-29`; each metric was
non-increasing in provider response order. Record and batch `source_at` were
absent as required. No diagnostic production-success marker was emitted.
The historical output above retains the label printed by that exact run. The
post-review source now prints `capability_admitted=true` so a failed fetch
cannot be misread as a successful batch admission.

## Production composition route live evidence

The provider-trait live result above does not by itself prove that the
zero-argument production composition Router admits the same batches. The
separate Gate D command is:

```bash
MAGIC_COMPOSITION_TOPN_DATE=<current-Asia/Shanghai-date> \
MAGIC_COMPOSITION_TOPN_LIMIT=20 \
MAGIC_COMPOSITION_TOPN_KIND=all \
cargo run -p magic-market-composition \
  --example provider_top_n_live_probe --locked
```

```text
=== composition_provider_top_n.VolumeRatio ===
kind=VolumeRatio
status=admitted
provider=Eastmoney
source=eastmoney-web
observed_at=2026-07-29T23:44:29+08:00
source_at=None
records=20
provider_declared_total=5542

=== composition_provider_top_n.MainNetInflow ===
kind=MainNetInflow
status=admitted
provider=Eastmoney
source=eastmoney-web
observed_at=2026-07-29T23:44:33+08:00
source_at=None
records=20
provider_declared_total=5542

failures=0
```

Both metrics traversed the zero-argument production composition constructor,
the concrete Router and the strict provider adapter. The first restricted
network attempt failed closed with two explicit exhausted-source errors; the
required real-network rerun produced the evidence above. No failed attempt was
admitted as an empty ranking.

## Final release gates

The final source state passed all required deterministic gates:

```text
cargo fmt --all -- --check
  PASS
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
  PASS (zero warnings)
cargo test --workspace --all-targets --all-features --locked --offline -- --test-threads=1
  PASS
bash tools/compliance/check.sh
  PASS
bash tools/docs/check_links.sh
  PASS
git diff --check
  PASS
```

Coverage was generated from a fresh complete workspace run and checked by the
repository threshold script:

```text
overall: covered=46108 total=52178 percent=88.37 required=80.00
critical: covered=27200 total=28626 percent=95.02 required=95.00
```

The release preflight was then executed with coverage evidence mandatory:

```bash
MAGIC_COVERAGE_JSON=target/coverage/coverage.json \
MAGIC_REQUIRE_COVERAGE_EVIDENCE=1 \
bash tools/release/preflight.sh
```

It rebuilt in an isolated temporary target directory and passed workspace
check, tests, Clippy with warnings denied, Rustdoc with warnings denied,
doc-tests, documentation links, compliance, coverage thresholds and diff
validation. The two explicitly ignored exchange integration tests require
real SSE/SZSE HTTPS and were not counted as passed.

The release package now contains 49 probes, including
`magic-provider-topn-live-probe`; its optimized locked/offline build passed.
Provider-declared total remains positive typed evidence rather than an
unregistered rejection threshold, and a deterministic test admits a total of
20,001 while still enforcing the requested two-row page cardinality.

## Admission decision

The zero-argument production-composition command returned both metrics with
`failures=0`, so this release admits
`ProviderTopNRankingCapabilities.volume_ratio=true` and
`main_net_inflow=true`. `MarketRankingCapabilities` and
`SignalCapabilities.market_rankings` remain false.

Downstream wording is limited to:

> Eastmoney single-response provider-ordered Top-N with the requested metric
> present in every returned row.
