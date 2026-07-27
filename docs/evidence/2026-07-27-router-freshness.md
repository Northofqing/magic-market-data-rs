# Strict realtime Router freshness evidence — 2026-07-27

This artifact records deterministic and bounded live evidence for the optional
five-second realtime source-time policy. It applies only during continuous
trading and never substitutes local `observed_at` for provider `source_at`.

## Deterministic contract

```bash
cargo test -p magic-market-router --test freshness --locked --offline
cargo test -p magic-market-router --locked --offline
cargo clippy -p magic-market-router --all-targets --locked --offline -- -D warnings
```

Results:

```text
freshness: 11 passed
full Router package: all tests passed
all-target Clippy with -D warnings: passed
```

The tests cover exactly 5 seconds, 6 seconds, 5.001 seconds, future,
malformed, missing and ambiguous timestamps, record/batch disagreement, the
oldest-record rule, millisecond precision, timezone-equivalent instants,
zero-age configuration and a route without a freshness policy.

Before continuous trading, the release example exited successfully without
contacting a provider:

```text
freshness_policy=not_run session=unspecified
router_live_probe_status=skipped_non_continuous_session
```

## Continuous-trading live probe

Started at `2026-07-27T13:01:20+0800` (`Asia/Shanghai`) for liquid A-share
`600519.SH`:

```bash
MAGIC_ROUTER_SESSION=continuous \
MAGIC_ROUTER_CODE=600519.SH \
cargo run -p magic-market-router --example live_probe --release --locked --offline
```

The command completed successfully at `2026-07-27T13:01:32+0800`:

```text
freshness_policy=continuous_trading max_source_age_ms=5000
router providers=[Tdx, Tencent] require_complete=true require_source_at=true max_source_age=Some(5s)
attempt provider=Tdx status=Rejected { kind: Quality, message: "batch quality is incomplete: 600519: one or more normalized quote fields unavailable; 600519: security name unavailable from the TDX quote packet; 600519: TDX quote source timestamp format is unverified" }
attempt provider=Tencent status=Selected
selected_provider=Tencent records=1 provenance=Provenance { source: "tencent-web", source_at: Some("2026-07-27T13:01:29+08:00"), fetched_at: "1785128492.613522000", batch_id: Some("tencent-web:1785128492.613522000:quote") } quality=QualityReport { complete: true, issues: [] }
quote code=600519 price=1290.2 source_at=2026-07-27T13:01:29+08:00 observed_at=1785128492.613522000 source_age_ms=3613 provider=Tencent batch_id=tencent-web:1785128492.613522000:quote
router_live_probe_status=passed
```

## Decision

The strict continuous-session route passed with a measured source age of
`3613 ms`, below the inclusive `5000 ms` maximum. The first provider was not
silently accepted: TDX remained available to non-strict consumers but was
rejected here because its Quote was incomplete and its source timestamp was
unverified. Tencent was selected only after its complete, source-timestamped
batch passed the policy.

This result proves the bounded route at the recorded time; it is not a provider
SLA and does not authorize applying the five-second policy to lunch, pre-open,
post-close, replay or historical data.
