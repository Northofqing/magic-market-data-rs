# Eastmoney Provider Top-N Production Composition Probe Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a bounded live-evidence executable that proves both admitted Eastmoney Provider Top-N metrics through the zero-argument production composition Router.

**Architecture:** A standalone `magic-market-composition` example owns environment parsing, constructs `EastmoneyProviderTopNRankingRouter::new()`, and routes the two admitted metrics once each. Pure parsing and rendering helpers are tested under `cargo test --all-targets`; no public library API or transport injection is added.

**Tech Stack:** Rust 2021, `magic-market-composition`, `magic-eastmoney-rs`, `magic-market-core`, `time`.

---

### Task 1: Lock the bounded probe contract

**Files:**
- Create: `crates/magic-market-composition/examples/provider_top_n_live_probe.rs`

- [x] **Step 1: Write failing example tests**

Add tests around a private `ProbePlan::from_values(date, limit, kind)`:

```rust
#[test]
fn probe_plan_accepts_only_the_bounded_all_metric_plan() {
    let plan = ProbePlan::from_values(Some("2026-07-29"), Some("20"), Some("all")).unwrap();
    assert_eq!(plan.trading_date.as_str(), "2026-07-29");
    assert_eq!(plan.limit.get(), 20);
    assert!(ProbePlan::from_values(Some("2026-07-29"), Some("101"), Some("all")).is_err());
    assert!(ProbePlan::from_values(
        Some("2026-07-29"),
        Some("20"),
        Some("volume-ratio")
    )
    .is_err());
}
```

- [x] **Step 2: Run the example target and verify RED**

Run:

```bash
cargo test -p magic-market-composition --example provider_top_n_live_probe --locked --offline
```

Expected: compilation fails because `ProbePlan` is not implemented.

- [x] **Step 3: Implement the bounded plan**

Implement `ProbePlan` with:

```rust
struct ProbePlan {
    trading_date: IsoDate,
    limit: PositiveU32,
}

impl ProbePlan {
    fn from_values(
        date: Option<&str>,
        limit: Option<&str>,
        kind: Option<&str>,
    ) -> Result<Self, Box<dyn Error>> {
        if kind.unwrap_or("all") != "all" {
            return Err("MAGIC_COMPOSITION_TOPN_KIND must be exactly all".into());
        }
        let trading_date = IsoDate::new(match date {
            Some(value) => value.to_owned(),
            None => current_china_date(),
        })?;
        let limit = PositiveU32::new(limit.unwrap_or("20").parse()?)?;
        if limit.get() > ProviderTopNRankingRequest::MAX_SINGLE_PAGE_LIMIT {
            return Err("MAGIC_COMPOSITION_TOPN_LIMIT exceeds the single-page cap".into());
        }
        Ok(Self {
            trading_date,
            limit,
        })
    }
}
```

- [x] **Step 4: Run the example tests and verify GREEN**

Run the command from Step 2. Expected: all example tests pass without network.

### Task 2: Route both metrics through production composition

**Files:**
- Modify: `crates/magic-market-composition/examples/provider_top_n_live_probe.rs`

- [x] **Step 1: Add the production probe runner**

`main` must construct only:

```rust
let router = EastmoneyProviderTopNRankingRouter::new()?;
```

It must iterate exactly:

```rust
[
    MarketRankingKind::VolumeRatio,
    MarketRankingKind::MainNetInflow,
]
```

For every route, print `provider`, `source`, `observed_at`, `source_at`,
`records`, and `provider_declared_total`. Increment a failure counter on every
error, continue through both metrics, print `failures=<n>`, and return an error
when `n > 0`.

- [x] **Step 2: Compile and test every composition target**

Run:

```bash
cargo test -p magic-market-composition --all-targets --locked --offline
```

Expected: all unit, integration and example tests pass without network.

### Task 3: Document the operator command and evidence placeholder

**Files:**
- Modify: `crates/magic-market-composition/README.md`
- Modify: `docs/evidence/2026-07-29-eastmoney-provider-topn-rankings.md`

- [x] **Step 1: Document the bounded command**

Document:

```bash
MAGIC_COMPOSITION_TOPN_DATE=<current-Asia/Shanghai-date> \
MAGIC_COMPOSITION_TOPN_LIMIT=20 \
MAGIC_COMPOSITION_TOPN_KIND=all \
cargo run -p magic-market-composition \
  --example provider_top_n_live_probe --locked
```

State that the command always routes both metrics, performs real network I/O,
and has no transport injection.

- [x] **Step 2: Add an unambiguous evidence placeholder**

Record the exact command and `status=pending_not_run` without fabricated
provider output. Replace the placeholder only after the separately authorized
same-day live run.

- [ ] **Step 3: Run local gates**

```bash
cargo test -p magic-market-composition --all-targets --locked --offline
cargo fmt --all -- --check
git diff --check
```

Expected: all commands pass.
