# Official Data Integration and Release Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan
> task-by-task. Use requesting-code-review before the release commit and
> finishing-a-development-branch after every gate passes.

**Goal:** Add provider-neutral Router adapters, register exact documentation,
compliance, coverage, and package boundaries, then prove the complete committed
workspace through Gates A through D.

**Architecture:** Router depends only on Core and validates family-specific
identity/order/range facts before generic failover. Existing
`GlobalNewsRouter` is reused. Documentation and tooling read actual capability
constants/admission evidence; unsupported NBS, DR007, World Bank, or
auth-unconfigured FRED states remain visible. Release artifacts package probes
and tracked documentation from a clean commit.

**Tech Stack:** Rust Router/Core, shell compliance/package/preflight tools,
`cargo-llvm-cov 0.8.7` when already provisioned, Markdown link checker.

---

## Task 1: Add economic/reference/fixing Router adapters

**Files:**

- Create: `crates/magic-market-router/src/macro_data.rs`
- Create: `crates/magic-market-router/tests/macro_data_routing.rs`
- Modify: `crates/magic-market-router/src/lib.rs`

**Step 1: Write red tests for exact identity routing**

Define fixture Providers implementing the three Core traits. Test:

- economic batches may contain a subset of periods but every series key must
  be requested, periods must be in range/frequency, row count must not exceed
  `max_rows`, and `(series, region_code, period)` must be unique;
- economic rows sort by request-series position, region code/name, increasing
  period;
- reference-rate rows use only requested identities and in-range dates, are
  unique by `(identity, date)`, and sort request identity then date;
- official-fixing rows use only requested pairs and in-range dates, are unique
  by `(pair, date)`, and sort request identity then date;
- wrong record Provider or batch ID reaches the generic evidence rejection;
- a typed recoverable failure tries the next source;
- no default route substitutes a FRED key for an IMF/PBC key.

Run:

```bash
cargo test -p magic-market-router --test macro_data_routing --offline
```

Expected: unresolved adapters/type aliases.

**Step 2: Add public router aliases**

```rust
pub type EconomicSeriesRouter =
    FailoverChain<EconomicSeriesRequest, EconomicObservation>;
pub type ReferenceRateRouter =
    FailoverChain<ReferenceRateRequest, ReferenceRateObservation>;
pub type OfficialFxFixingRouter =
    FailoverChain<OfficialFxFixingRequest, OfficialFxFixing>;
```

**Step 3: Implement adapter functions**

Follow the existing closure-backed pattern:

```rust
pub fn economic_series_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<EconomicSeriesRequest, EconomicObservation>
where
    Provider: EconomicSeriesProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.economic_series(request).map_err(&classify)?;
        validate_economic_batch(request, batch.records())?;
        Ok(batch)
    })
}
```

Implement equivalent reference/fixing functions. Use `FailureKind::Evidence`
for unrequested/wrong-range facts and `FailureKind::Quality` for duplicates,
cardinality, or ordering. Do not compare or rewrite Provider evidence here;
the generic chain already does that.

Export aliases/functions from `lib.rs`.

**Step 4: Pass and commit**

```bash
cargo test -p magic-market-router --test macro_data_routing --offline
cargo clippy -p magic-market-router --all-targets --offline -- -D warnings
git add crates/magic-market-router/src/macro_data.rs \
  crates/magic-market-router/src/lib.rs \
  crates/magic-market-router/tests/macro_data_routing.rs
git commit -m "feat(router): add official macro data routes"
```

## Task 2: Add filing routing and new news identities

**Files:**

- Create: `crates/magic-market-router/src/filings.rs`
- Create: `crates/magic-market-router/tests/filing_routing.rs`
- Modify: `crates/magic-market-router/tests/intelligence_routing.rs`
- Modify: `crates/magic-market-router/src/lib.rs`

**Step 1: Write filing adapter red tests**

Prove:

- returned companies are requested;
- form/date filters are honored;
- record count does not exceed `max_records`;
- `(CIK, accession)` is unique;
- records sort by requested company position, filing date descending,
  acceptance time descending;
- a record whose optional ticker contradicts the request fails evidence;
- wrong provider/batch evidence is rejected by generic routing;
- the only phase-one route is SEC unless a caller explicitly registers a
  separate conforming Provider.

Run:

```bash
cargo test -p magic-market-router --test filing_routing --offline
```

Expected: unresolved adapter/type alias.

**Step 2: Implement filing adapter**

```rust
pub type CompanyFilingRouter =
    FailoverChain<CompanyFilingRequest, CompanyFiling>;

pub fn company_filing_source<Provider, Classify>(
    provider_id: ProviderId,
    provider: Arc<Provider>,
    classify: Classify,
) -> SourceFn<CompanyFilingRequest, CompanyFiling>
where
    Provider: CompanyFilingsProvider + Send + Sync + 'static,
    Classify: Fn(Provider::Error) -> SourceError + Send + Sync + 'static,
{
    SourceFn::new(provider_id, move |request| {
        let batch = provider.company_filings(request).map_err(&classify)?;
        validate_filing_batch(request, batch.records())?;
        Ok(batch)
    })
}
```

Export it from `lib.rs`.

**Step 3: Extend existing global-news routing tests**

Use the existing `NewsFixtureProvider` to add:

```rust
#[test]
fn global_news_router_accepts_new_metadata_only_identities() {
    for provider_id in [
        ProviderId::XinhuaFinance,
        ProviderId::Yicai,
        ProviderId::SecuritiesTimes,
    ] {
        let provider = Arc::new(NewsFixtureProvider {
            record_provider: provider_id,
            batch_source: "first-party-metadata-v1",
            item_count: 1,
            duplicate_id: false,
        });
        let mut router =
            GlobalNewsRouter::new(AcceptancePolicy::new().with_require_source_at(true));
        router
            .register(global_news_source(provider_id, provider, classify))
            .unwrap();
        assert_eq!(
            router.route(&PositiveU32::new(1).unwrap()).unwrap().selected_provider(),
            provider_id,
        );
    }
}
```

Run:

```bash
cargo test -p magic-market-router --test filing_routing --offline
cargo test -p magic-market-router --test intelligence_routing \
  global_news_router_accepts_new_metadata_only_identities --offline
```

Expected: pass.

**Step 4: Commit**

```bash
git add crates/magic-market-router/src/filings.rs \
  crates/magic-market-router/src/lib.rs \
  crates/magic-market-router/tests/filing_routing.rs \
  crates/magic-market-router/tests/intelligence_routing.rs
git commit -m "feat(router): route filings and new financial news"
```

## Task 3: Register source integration documents and business rules

**Files:**

- Modify: `README.md`
- Modify: `docs/DEPLOYMENT.md`
- Modify: `docs/UPSTREAM.md`
- Modify: `docs/business_rules.md`
- Create: `docs/integrations/nbs-official.md`
- Create: `docs/integrations/pbc-official.md`
- Create: `docs/integrations/cfets-official.md`
- Create: `docs/integrations/fred-api.md`
- Create: `docs/integrations/imf-datamapper.md`
- Create: `docs/integrations/worldbank-indicators.md`
- Create: `docs/integrations/sec-edgar.md`
- Create: `docs/integrations/xinhua-finance.md`
- Create: `docs/integrations/yicai-news.md`
- Create: `docs/integrations/securities-times.md`
- Create: `docs/evidence/2026-07-29-official-macro-global-news.md`

**Step 1: Add four contiguous business rules**

Append:

```markdown
## BR-039 Official economic observation integrity

Provider-native namespace/code/region/frequency/period/unit/scale/revision
facts remain source-scoped. Missing is never zero, local fetch time is never a
release time, and any failed page/series invalidates the atomic request.

## BR-040 Official rate and fixing identity

Benchmark tenor, percent unit, base/quote orientation, quotation base and
fixing date are mandatory source facts. DR007, R007, Shibor and LPR are not
interchangeable, and an official fixing is not a realtime quote.

## BR-041 SEC filing metadata-only access

SEC requests use official submissions hosts, a descriptive redacted
User-Agent, bounded pacing and atomic older-file composition. Normalized
records expose metadata/canonical links only and never download bodies,
attachments or XBRL facts.

## BR-042 Public financial-news metadata boundary

Xinhua Finance, Yicai and Securities Times records retain only first-party
title/ID/link/publisher/publication-time/topic metadata. Bodies, descriptions,
images, login state, cookies and inferred instruments are prohibited.
```

**Step 2: Write one integration document per source**

Every document uses the same explicit headings:

```markdown
# Source name integration

## Capability state
## Official host and paths
## Request and response ceilings
## Identity, unit, missing, and source-time semantics
## Authentication or usage-rights boundary
## Deterministic tests
## Live and load admission evidence
## Explicit unsupported operations
```

Populate each heading with the exact constants and actual Task admission
results. NBS describes diagnostic-only 403 evidence. CFETS describes false
DR007 and operator rights obligations. FRED describes runtime-key
configuration. World Bank describes the structured-unit blocker. News
documents state metadata-only.

**Step 3: Update README everywhere it duplicates state**

Update:

- current-state date/prose;
- workspace crate table;
- dependency diagram, including
  `magic-market-transport -> magic-market-core`;
- contract/family table;
- capability matrix using actual constants;
- environment variables (`FRED_API_KEY`, `SEC_USER_AGENT`);
- deterministic/live/load commands;
- release package binary tree;
- residual-gap list.

Use stock code plus stock name convention only where records actually contain
an instrument; macro/news/filing rows must not fabricate an `InstrumentId`.

**Step 4: Write the evidence artifact**

Record for each source:

- exact command and timestamp;
- production-client result category;
- records and source/observed time availability;
- admission flag set;
- residual unsupported reason.

Do not include API keys, User-Agent contact, article descriptions, or full
response bodies.

**Step 5: Pass docs checks and commit**

```bash
bash tools/docs/check_links.sh
rg -n 'secret-key-value|operations@example.com' \
  README.md docs/DEPLOYMENT.md docs/UPSTREAM.md docs/business_rules.md \
  docs/integrations docs/evidence crates/*/README.md
```

Expected: link check passes and `rg` returns no matches.

```bash
git add README.md docs crates/*/README.md
git commit -m "docs: document official macro SEC and news providers"
```

## Task 4: Register compliance, coverage, and packaged probes

**Files:**

- Modify: `tools/compliance/check.sh`
- Modify: `tools/coverage/check_thresholds.py`
- Modify: `tools/coverage/test_check_thresholds.py`
- Modify: `tools/release/package.sh`
- Modify: `docs/DEPLOYMENT.md`

**Step 1: Extend compliance red expectations**

Add the transport and ten Provider manifests/integration docs to `required`.
Add all eleven crate directories to `workspace_members`. Extend Router's
forbidden concrete Provider dependency regex with:

```text
nbs|pbc|cfets|fred|imf|worldbank|sec|xinhua|yicai|stcn
```

Change the rule loop to:

```bash
for number in $(seq 1 42); do
```

Run:

```bash
bash tools/compliance/check.sh
```

Expected: pass with every registration present.

**Step 2: Add critical coverage globs**

Append to `CRITICAL_GLOBS`:

```python
    "crates/magic-market-transport/src/*.rs",
    "crates/magic-nbs-rs/src/*.rs",
    "crates/magic-pbc-rs/src/*.rs",
    "crates/magic-cfets-rs/src/*.rs",
    "crates/magic-fred-rs/src/*.rs",
    "crates/magic-imf-rs/src/*.rs",
    "crates/magic-worldbank-rs/src/*.rs",
    "crates/magic-sec-rs/src/*.rs",
    "crates/magic-xinhua-rs/src/*.rs",
    "crates/magic-yicai-rs/src/*.rs",
    "crates/magic-stcn-rs/src/*.rs",
```

Run:

```bash
python3 -m unittest discover -s tools/coverage -p 'test_*.py' -v
```

Expected: pass. Keep tests outside critical `src` files so test bodies cannot
inflate production coverage.

**Step 3: Register probe binaries**

Add `build_probe` calls:

```bash
build_probe magic-nbs-rs live_probe magic-nbs-live-probe
build_probe magic-pbc-rs live_probe magic-pbc-live-probe
build_probe magic-pbc-rs load_probe magic-pbc-load-probe
build_probe magic-cfets-rs live_probe magic-cfets-live-probe
build_probe magic-cfets-rs load_probe magic-cfets-load-probe
build_probe magic-fred-rs live_probe magic-fred-live-probe
build_probe magic-fred-rs load_probe magic-fred-load-probe
build_probe magic-imf-rs live_probe magic-imf-live-probe
build_probe magic-imf-rs load_probe magic-imf-load-probe
build_probe magic-worldbank-rs live_probe magic-worldbank-live-probe
build_probe magic-sec-rs live_probe magic-sec-live-probe
build_probe magic-sec-rs load_probe magic-sec-load-probe
build_probe magic-xinhua-rs live_probe magic-xinhua-live-probe
build_probe magic-xinhua-rs load_probe magic-xinhua-load-probe
build_probe magic-yicai-rs live_probe magic-yicai-live-probe
build_probe magic-yicai-rs load_probe magic-yicai-load-probe
build_probe magic-stcn-rs live_probe magic-stcn-live-probe
build_probe magic-stcn-rs load_probe magic-stcn-load-probe
```

Update deployment binary lists and commands to match.

**Step 4: Pass tooling tests and commit**

```bash
bash -n tools/compliance/check.sh tools/release/package.sh \
  tools/release/preflight.sh
bash tools/compliance/check.sh
python3 -m unittest discover -s tools/coverage -p 'test_*.py' -v
git diff --check
git add tools docs/DEPLOYMENT.md
git commit -m "build: register official data release gates"
```

## Task 5: Close deterministic failures and coverage gaps

**Files:**

- Modify: only files named by failing commands

**Step 1: Run focused workspace gates**

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked --offline
cargo test --workspace --all-targets --all-features --locked --offline \
  -- --test-threads=1
cargo clippy --workspace --all-targets --all-features --locked --offline \
  -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc \
  --workspace --all-features --no-deps --locked --offline
cargo test --workspace --all-features --doc --locked --offline \
  -- --test-threads=1
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
git diff --check
```

Expected: every command passes. Diagnose each failure with the
`systematic-debugging` skill; do not weaken an invariant or capability flag to
make a test green.

**Step 2: Produce and validate coverage evidence**

```bash
cargo llvm-cov --version
cargo llvm-cov clean --workspace
mkdir -p target/coverage
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo llvm-cov \
  --workspace --all-features --locked --offline \
  --json --output-path target/coverage/coverage.json \
  -- --test-threads=1
python3 tools/coverage/check_thresholds.py target/coverage/coverage.json
```

Expected: overall production coverage at least 80% and critical aggregate at
least 95%. Add deterministic boundary tests for uncovered branches; do not
exclude new production files or add inline `#[cfg(test)]` bodies.

**Step 3: Commit any gate repairs**

```bash
git add crates tools docs README.md Cargo.toml Cargo.lock
git commit -m "test: close official data release coverage"
```

Skip the commit only if there are no changes.

## Task 6: Review, clean-tree package, and full release preflight

**Files:**

- Verify only unless review finds defects

**Step 1: Request code review**

Use `requesting-code-review` against the range from the foundation's first
commit through `HEAD`. Resolve every P0/P1 and rerun the focused command for
each changed family. Commit review repairs:

```bash
git add crates tools docs README.md Cargo.toml Cargo.lock
git commit -m "fix: resolve official data review findings"
```

**Step 2: Commit the final admission/gate evidence**

Update the planning progress and evidence artifact with source admission
results, deterministic gate results, compiler/Cargo versions, and coverage
ratios. Do not attempt to embed the commit's own future hash or a package path.

```bash
git add .planning/2026-07-29-official-macro-global-news \
  docs/evidence/2026-07-29-official-macro-global-news.md
git commit -m "chore: record official data release evidence"
```

**Step 3: Verify clean committed state**

```bash
git status --short
git diff --check
```

Expected: no status output and no diff errors.

**Step 4: Build release package from the final clean tree**

```bash
bash tools/release/package.sh
```

Expected: package script builds every registered probe and reports one
revision-scoped `target/dist/$REVISION` directory with checksums.

**Step 5: Run full preflight with coverage evidence required**

```bash
MAGIC_COVERAGE_JSON=target/coverage/coverage.json \
MAGIC_REQUIRE_COVERAGE_EVIDENCE=1 \
bash tools/release/preflight.sh
```

Expected: `release preflight: passed`.

Report the exact final `HEAD`, package path, checksum result, coverage ratios,
and preflight result to the user without modifying the clean tree. Then use
`finishing-a-development-branch` to present merge, push, or retention choices;
do not push or merge without the user's explicit selection.
