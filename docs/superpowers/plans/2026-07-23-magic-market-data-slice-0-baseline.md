# Magic Market Data Slice 0 Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the current uncommitted public-intelligence Provider work into one reviewable, version-aligned `magic-market-data-rs 0.2.0` baseline commit series with deterministic, live, load, compliance, coverage and release evidence.

**Architecture:** Slice 0 changes only the upstream Provider repository. Core retains provider-neutral contracts, Router retains provider-neutral failover, and six isolated read-only crates own public endpoint protocols. This plan does not create the downstream `stock_analysis::data_gateway`; it produces the exact upstream commit SHA that Slice 1 may consume.

**Tech Stack:** Rust 1.83, Cargo workspace, `magic-market-core`, `magic-market-router`, `reqwest` blocking transports, Bash compliance/release scripts, `cargo-llvm-cov`, Git and GitHub CLI.

---

## Approved design and execution context

- Overall design: `stock_analysis` commit `8fe06b0`,
  `docs/superpowers/specs/2026-07-23-magic-market-data-unified-gateway-design.md`.
- Execution repository:
  `/Users/zhangzhen/Desktop/Quant/magic-market-data-rs`.
- Starting committed upstream revision: `21787ed`.
- The working tree already contains the intended Core/Router widening and six
  untracked Provider crates. Treat those files as implementation input that
  still requires independent verification; their presence is not completion.
- Preserve the user's untracked
  `docs/integrations/stock-analysis-market-data-requirements.md`. Do not stage,
  edit or delete it in Slice 0.

### Upstream debt

- The six public Provider crates are untracked, so release packaging correctly
  rejects the current tree.
- `magic-market-analysis` is `0.1.0` while every other workspace crate is
  `0.2.0`.
- Final CLS/Baidu/iWencai remediation is present in the working tree but has not
  passed the final root preflight as a committed release candidate.
- No downstream `stock_analysis` production path consumes these providers yet;
  that is Slice 1, not evidence for Slice 0.

### Rename impact

- No public Rust identifier is renamed in Slice 0.
- Changing `magic-market-analysis` from `0.1.0` to `0.2.0` requires the
  workspace-version compliance check and lockfile verification.
- Provider capability flags must remain conservative. In particular,
  Eastmoney fund-flow series, Eastmoney post-close Top10 and iWencai semantic
  search remain unadvertised until their own real admission evidence exists.
- `stock_analysis` BR-158/BR-159 govern the downstream Gateway and batch-log
  behavior beginning in Slice 1. Slice 0 changes no downstream behavior, so it
  first registers upstream Provider rules BR-009 through BR-011 in the Magic
  repository's independent rule namespace.

### Production evidence

Slice 0 evidence is upstream live Provider evidence, not a downstream push:

- Eastmoney, CNInfo, THS, CLS and Baidu live probes exit zero and print
  non-empty `DataBatch` evidence for every advertised family.
- Eastmoney unadmitted fund-flow diagnostics may fail explicitly without
  failing the admitted-capability probe.
- iWencai without an authorized key exits with typed, redacted
  `Authentication`; capabilities keep `semantic_search=false`.
- Every live record prints provider, source time when supplied, observation
  time and batch identity.

## File ownership map

| Path | Responsibility |
| --- | --- |
| `crates/magic-market-core/src/{capital,content,limit_pool,research,signals}.rs` | Provider-neutral records, requests, capabilities and checked construction |
| `crates/magic-market-router/src/adapters.rs` | Provider-neutral routing aliases only |
| `crates/magic-eastmoney-rs/` | Eastmoney research, capital, signals, limit pools and instrument news |
| `crates/magic-cninfo-rs/` | CNInfo announcements and investor questions |
| `crates/magic-ths-rs/` | THS consensus, strong-stock reasons, limit-up pool and popularity |
| `crates/magic-cls-rs/` | CLS global telegraph news |
| `crates/magic-baidu-rs/` | Baidu unadjusted daily bars and source MA values |
| `crates/magic-iwencai-rs/` | Authorized semantic search with conservative capability admission |
| `tools/compliance/check.sh` | Required files, provider neutrality and uniform crate-version checks |
| `tools/release/preflight.sh` | Clean isolated Rust 1.83 Gate C execution |
| `tools/release/package.sh` | Reproducible probe and documentation package |
| `docs/integrations/*.md` | Endpoint, authorization, units, limits and evidence contracts |
| `docs/PERFORMANCE_RESULTS.md` | Timestamped bounded live/load evidence |

## Task 1: Protect the dirty implementation and verify scope

**Files:**

- Inspect only: entire repository
- Preserve: `docs/integrations/stock-analysis-market-data-requirements.md`

- [ ] **Step 1: Create the implementation branch without discarding changes**

Run:

```bash
git branch --show-current
git switch -c feat/public-intelligence-v0.2
```

Expected: the first command prints `main`; the switch succeeds and all tracked
and untracked changes remain present.

- [ ] **Step 2: Record the exact intended change classes**

Run:

```bash
git status --short
git diff --check
```

Expected: modified Core/Router/docs/tooling files plus exactly six new Provider
crate directories and six new integration guides; `git diff --check` exits
zero.

- [ ] **Step 3: Prove the user handoff remains untracked**

Run:

```bash
git ls-files --error-unmatch docs/integrations/stock-analysis-market-data-requirements.md
```

Expected: non-zero exit with “pathspec did not match any file”.

- [ ] **Step 4: Prove release packaging currently fails closed**

Run:

```bash
bash tools/release/package.sh
```

Expected: non-zero exit identifying dirty tracked state or an untracked build
input. A successful package at this point is a defect.

## Task 2: Adopt and commit the provider-neutral Core and Router contracts

**Files:**

- Modify/adopt: `crates/magic-market-core/src/capital.rs`
- Modify/adopt: `crates/magic-market-core/src/content.rs`
- Modify/adopt: `crates/magic-market-core/src/lib.rs`
- Modify/adopt: `crates/magic-market-core/src/limit_pool.rs`
- Modify/adopt: `crates/magic-market-core/src/research.rs`
- Modify/adopt: `crates/magic-market-core/src/signals.rs`
- Modify/adopt: `crates/magic-market-core/tests/capital.rs`
- Modify/adopt: `crates/magic-market-core/tests/content.rs`
- Modify/adopt: `crates/magic-market-core/tests/limit_pool.rs`
- Modify/adopt: `crates/magic-market-core/tests/research.rs`
- Modify/adopt: `crates/magic-market-core/tests/signals.rs`
- Modify/adopt: `crates/magic-market-core/tests/sourced_record.rs`
- Modify/adopt: `crates/magic-market-router/src/adapters.rs`
- Modify/adopt: `crates/magic-market-router/src/lib.rs`
- Modify/adopt: `crates/magic-market-router/tests/adapters.rs`
- Modify/adopt: `crates/magic-market-router/tests/intelligence_routing.rs`
- Modify/adopt: `crates/magic-market-analysis/tests/analysis.rs`

- [ ] **Step 1: Run the focused contract tests**

Run:

```bash
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-market-core --all-targets --locked --offline
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-market-router --all-targets --locked --offline
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-market-analysis --all-targets --locked --offline
```

Expected: all three commands exit zero. Tests must include checked construction,
legacy deserialization, record evidence and non-empty PostClose routing.

- [ ] **Step 2: Run focused strict Clippy**

Run:

```bash
RUSTUP_TOOLCHAIN=1.83.0 cargo clippy \
  -p magic-market-core -p magic-market-router -p magic-market-analysis \
  --all-targets --locked --offline -- -D warnings
```

Expected: exit zero with no warning.

- [ ] **Step 3: Stage only the provider-neutral contract change**

Run:

```bash
git add \
  crates/magic-market-core/src/capital.rs \
  crates/magic-market-core/src/content.rs \
  crates/magic-market-core/src/lib.rs \
  crates/magic-market-core/src/limit_pool.rs \
  crates/magic-market-core/src/research.rs \
  crates/magic-market-core/src/signals.rs \
  crates/magic-market-core/tests/capital.rs \
  crates/magic-market-core/tests/content.rs \
  crates/magic-market-core/tests/limit_pool.rs \
  crates/magic-market-core/tests/research.rs \
  crates/magic-market-core/tests/signals.rs \
  crates/magic-market-core/tests/sourced_record.rs \
  crates/magic-market-router/src/adapters.rs \
  crates/magic-market-router/src/lib.rs \
  crates/magic-market-router/tests/adapters.rs \
  crates/magic-market-router/tests/intelligence_routing.rs \
  crates/magic-market-analysis/tests/analysis.rs
git diff --cached --check
```

Expected: cached diff contains no concrete Provider dependency and whitespace
check exits zero.

- [ ] **Step 4: Commit the contract barrier**

Run:

```bash
git commit -m "feat(core): add public intelligence contracts"
```

Expected: one commit containing only Core, Router and provider-neutral analysis
tests.

## Task 3: Register public-provider business rules before adoption

**Files:**

- Modify: `docs/business_rules.md`

- [ ] **Step 1: Register capability, pacing and duplicate governance**

Append the following exact rules to `docs/business_rules.md`:

```markdown
## BR-009 Public-provider capability admission
An optional public-web capability is advertised only after deterministic
contract tests and a bounded live probe both prove the normalized records,
source identity, source time when supplied, observation time and batch
identity. Authentication-gated or unverified families remain false and return
a typed `Authentication`, `Unsupported` or protocol error.

## BR-010 Public-provider request bounds and pacing
Every public-web request enforces its verified positive row bound before I/O.
Clones of one Provider client share the same request limiter. Where the source
contract requires pacing, request starts are serialized at no less than the
documented interval; HTTP 429 and limiter failure are explicit errors and do
not trigger an unpaced retry.

## BR-011 Public-provider duplicate identity
Within one atomic Provider batch, duplicate business identities are rejected
as protocol failures. The only admitted exception is semantic-search output:
rows with the same normalized security identity are collapsed to the
source-supplied highest score, with deterministic first-seen tie breaking.
No downstream consumer may deduplicate by display name.
```

- [ ] **Step 2: Verify the rules are registered before Provider code is staged**

Run:

```bash
rg -n '^## BR-00(9|10|11) ' docs/business_rules.md
git diff --check docs/business_rules.md
```

Expected: exactly three headings print and the whitespace check exits zero.

- [ ] **Step 3: Commit the business-rule barrier**

Run:

```bash
git add docs/business_rules.md
git diff --cached --check
git commit -m "docs: register public provider governance"
```

Expected: one documentation-only commit preceding every new Provider commit.

## Task 4: Enforce the uniform workspace version and adopt all six Providers

**Files:**

- Modify: `tools/compliance/check.sh`
- Modify: `crates/magic-market-analysis/Cargo.toml`
- Modify/adopt: `Cargo.toml`
- Modify/adopt: `Cargo.lock`
- Create/adopt: `crates/magic-eastmoney-rs/`
- Create/adopt: `crates/magic-cninfo-rs/`
- Create/adopt: `crates/magic-ths-rs/`
- Create/adopt: `crates/magic-cls-rs/`
- Create/adopt: `crates/magic-baidu-rs/`
- Create/adopt: `crates/magic-iwencai-rs/`

- [ ] **Step 1: Add a failing uniform-version compliance check**

Insert this block in `tools/compliance/check.sh` immediately after the
`workspace_members` membership loop:

```bash
expected_workspace_crate_version=0.2.0
while IFS= read -r manifest; do
  package_version=$(
    awk '
      /^\[package\]$/ { in_package=1; next }
      /^\[/ && in_package { exit }
      in_package && /^version = "/ {
        gsub(/^version = "|".*$/, "")
        print
        exit
      }
    ' "$manifest"
  )
  if [[ "$package_version" != "$expected_workspace_crate_version" ]]; then
    printf 'workspace crate version mismatch: %s expected=%s actual=%s\n' \
      "$manifest" "$expected_workspace_crate_version" "$package_version" >&2
    exit 1
  fi
done < <(find crates -mindepth 2 -maxdepth 2 -name Cargo.toml -print | LC_ALL=C sort)
```

- [ ] **Step 2: Run the compliance check to verify RED**

Run:

```bash
bash tools/compliance/check.sh
```

Expected: non-zero exit containing:

```text
workspace crate version mismatch: crates/magic-market-analysis/Cargo.toml expected=0.2.0 actual=0.1.0
```

- [ ] **Step 3: Align magic-market-analysis**

Change the package header in
`crates/magic-market-analysis/Cargo.toml` to:

```toml
[package]
name = "magic-market-analysis"
version = "0.2.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
description = "Deterministic provider-neutral analysis for magic market data"
```

- [ ] **Step 4: Run deterministic tests for every new Provider**

Run:

```bash
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-eastmoney-rs --all-targets --locked --offline
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-cninfo-rs --all-targets --locked --offline
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-ths-rs --all-targets --locked --offline
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-cls-rs --all-targets --locked --offline
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-baidu-rs --all-targets --locked --offline
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-iwencai-rs --all-targets --locked --offline
```

Expected: all commands exit zero. The suites must exercise strict empty
responses, exchange/code mismatch, completion-time observations, clone-shared
pacing and conservative capability declarations.

- [ ] **Step 5: Run strict Provider Clippy and rustdoc**

Run:

```bash
RUSTUP_TOOLCHAIN=1.83.0 cargo clippy \
  -p magic-eastmoney-rs -p magic-cninfo-rs -p magic-ths-rs \
  -p magic-cls-rs -p magic-baidu-rs -p magic-iwencai-rs \
  --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS="-D warnings" RUSTUP_TOOLCHAIN=1.83.0 cargo doc \
  -p magic-eastmoney-rs -p magic-cninfo-rs -p magic-ths-rs \
  -p magic-cls-rs -p magic-baidu-rs -p magic-iwencai-rs \
  --no-deps --locked --offline
```

Expected: both commands exit zero.

- [ ] **Step 6: Verify capability flags remain conservative**

Run:

```bash
rg -n \
  'fund_flow_series: false|post_close_flow: false|semantic_search: false|instrument_news: false' \
  crates/magic-{eastmoney,iwencai,cls}-rs/src
```

Expected: output proves Eastmoney unverified fund-flow/post-close,
iWencai unauthenticated semantic search and CLS instrument news are not
advertised.

- [ ] **Step 7: Stage the Provider baseline but leave compliance unstaged**

Run:

```bash
git add \
  Cargo.toml Cargo.lock \
  crates/magic-market-analysis/Cargo.toml \
  crates/magic-eastmoney-rs \
  crates/magic-cninfo-rs \
  crates/magic-ths-rs \
  crates/magic-cls-rs \
  crates/magic-baidu-rs \
  crates/magic-iwencai-rs
git diff --cached --check
git status --short tools/compliance/check.sh
```

Expected: the six complete crates, workspace manifest, lockfile and aligned
analysis manifest are staged; `tools/compliance/check.sh` remains modified but
unstaged.

- [ ] **Step 8: Commit the Provider baseline**

Run:

```bash
git commit -m "feat(providers): add public intelligence sources"
```

Expected: the commit is independently buildable on top of the Task 3
business-rule barrier.

## Task 5: Commit integration contracts and evidence documentation

**Files:**

- Modify/adopt: `README.md`
- Modify/adopt: `CHANGELOG.md`
- Modify/adopt: `crates/magic-market-core/README.md`
- Modify/adopt: `docs/DEPLOYMENT.md`
- Modify/adopt: `docs/MULTI_PROVIDER_ROUTING.md`
- Modify/adopt: `docs/PERFORMANCE_RESULTS.md`
- Modify/adopt: `docs/superpowers/plans/2026-07-23-public-intelligence-providers.md`
- Create/adopt: `docs/integrations/eastmoney-web.md`
- Create/adopt: `docs/integrations/cninfo-web.md`
- Create/adopt: `docs/integrations/tonghuashun-web.md`
- Create/adopt: `docs/integrations/cls-web.md`
- Create/adopt: `docs/integrations/baidu-web.md`
- Create/adopt: `docs/integrations/iwencai-api.md`
- Modify/adopt: `.planning/2026-07-23-a-stock-data-parity/task_plan.md`
- Modify/adopt: `.planning/2026-07-23-a-stock-data-parity/findings.md`
- Modify/adopt: `.planning/2026-07-23-a-stock-data-parity/progress.md`

- [ ] **Step 1: Verify every advertised family has an integration contract**

Run:

```bash
bash tools/docs/check_links.sh
rg -n '^## 已实现的数据族|^## 标准化字段|^## 标准化数据|^## 探针|^## 生产边界' \
  docs/integrations/{eastmoney-web,cninfo-web,tonghuashun-web,cls-web,baidu-web,iwencai-api}.md
```

Expected: link check exits zero and every guide documents fields, probes and
production limits.

- [ ] **Step 2: Verify documentation does not overclaim unadmitted abilities**

Run:

```bash
rg -n 'post_close_flow.*false|fund_flow_series.*false|semantic_search.*false|Unsupported|Authentication' \
  README.md docs crates/magic-{eastmoney,iwencai,cls}-rs/tests
```

Expected: the docs and capability tests explicitly describe unadmitted or
authentication-gated abilities.

- [ ] **Step 3: Stage only documentation and planning evidence**

Run:

```bash
git add \
  README.md CHANGELOG.md \
  crates/magic-market-core/README.md \
  docs/DEPLOYMENT.md docs/MULTI_PROVIDER_ROUTING.md docs/PERFORMANCE_RESULTS.md \
  docs/superpowers/plans/2026-07-23-public-intelligence-providers.md \
  docs/integrations/eastmoney-web.md \
  docs/integrations/cninfo-web.md \
  docs/integrations/tonghuashun-web.md \
  docs/integrations/cls-web.md \
  docs/integrations/baidu-web.md \
  docs/integrations/iwencai-api.md \
  .planning/2026-07-23-a-stock-data-parity/task_plan.md \
  .planning/2026-07-23-a-stock-data-parity/findings.md \
  .planning/2026-07-23-a-stock-data-parity/progress.md
git diff --cached --check
git status --short docs/integrations/stock-analysis-market-data-requirements.md
```

Expected: the user handoff is still `??` and is not in the cached diff.

- [ ] **Step 4: Commit the documentation**

Run:

```bash
git commit -m "docs: document public intelligence providers"
```

Expected: documentation and evidence only.

## Task 6: Commit CI, compliance and packaging gates

**Files:**

- Modify/adopt: `.github/workflows/live-and-bench.yml`
- Modify/adopt: `tools/compliance/check.sh`
- Modify: `tools/release/preflight.sh`
- Modify/adopt: `tools/release/package.sh`

- [ ] **Step 1: Prove the current preflight does not exercise all features**

Run:

```bash
if rg -U 'cargo (check|test|clippy)[\\s\\\\]*.*--all-features' \
  tools/release/preflight.sh; then
  printf 'unexpected: all-feature preflight already present\n' >&2
  exit 1
fi
```

Expected: exit zero because the starting preflight omits `--all-features`.

- [ ] **Step 2: Add all-feature execution to every compile gate**

In `tools/release/preflight.sh`, replace the five Cargo command bodies with:

```bash
CARGO_TARGET_DIR="$preflight_target_dir" RUSTUP_TOOLCHAIN=1.83.0 \
  cargo check --workspace --all-targets --all-features --locked --offline
CARGO_TARGET_DIR="$preflight_target_dir" RUSTUP_TOOLCHAIN=1.83.0 \
  cargo test --workspace --all-targets --all-features --locked --offline \
  -- --test-threads=1
CARGO_TARGET_DIR="$preflight_target_dir" RUSTUP_TOOLCHAIN=1.83.0 \
  cargo clippy --workspace --all-targets --all-features --locked --offline \
  -- -D warnings
CARGO_TARGET_DIR="$preflight_target_dir" RUSTDOCFLAGS='-D warnings' RUSTUP_TOOLCHAIN=1.83.0 \
  cargo doc --workspace --all-features --no-deps --locked --offline
CARGO_TARGET_DIR="$preflight_target_dir" RUSTUP_TOOLCHAIN=1.83.0 \
  cargo test --workspace --all-features --doc --locked --offline \
  -- --test-threads=1
```

Expected: the preflight now matches the approved design's all-feature
validation boundary and serializes tests.

- [ ] **Step 3: Make compliance verify the new business-rule registrations**

Replace the final single-rule check in `tools/compliance/check.sh` with:

```bash
for rule_id in BR-001 BR-002 BR-009 BR-010 BR-011; do
  rg -q "^## $rule_id " docs/business_rules.md || {
    echo "missing registered business rule: $rule_id" >&2
    exit 1
  }
done
rg -q '^## Gate D ' docs/ENGINEERING_RULES.md
```

- [ ] **Step 4: Verify uniform version, rules and all-feature preflight are GREEN**

Run:

```bash
bash tools/compliance/check.sh
bash tools/release/preflight.sh
```

Expected: both commands exit zero; every workspace crate reports version
`0.2.0`, BR-009 through BR-011 are present, and all features pass.

- [ ] **Step 5: Validate release shell syntax**

Run:

```bash
bash -n tools/compliance/check.sh tools/release/preflight.sh tools/release/package.sh
```

Expected: exit zero.

- [ ] **Step 6: Verify packaging includes all live/load probes**

Run:

```bash
rg -n 'build_probe magic-(eastmoney|cninfo|ths|cls|baidu|iwencai)-rs' tools/release/package.sh
```

Expected: twelve lines, one live and one load probe for each of the six
Provider crates.

- [ ] **Step 7: Stage and commit only release governance**

Run:

```bash
git add \
  .github/workflows/live-and-bench.yml \
  tools/compliance/check.sh \
  tools/release/preflight.sh \
  tools/release/package.sh
git diff --cached --check
git commit -m "ci: gate public intelligence release"
```

Expected: one CI/compliance/release-tooling commit.

## Task 7: Run the deterministic Gate C and coverage Gate D

**Files:**

- No planned file changes

- [ ] **Step 1: Confirm intended tracked work is committed**

Run:

```bash
git status --short
```

Expected: only
`?? docs/integrations/stock-analysis-market-data-requirements.md`.

- [ ] **Step 2: Run the isolated release preflight**

Run:

```bash
bash tools/release/preflight.sh
```

Expected: exit zero and final line `release preflight: passed`.

- [ ] **Step 3: Produce and enforce the workspace coverage artifact**

Run:

```bash
mkdir -p target/coverage
RUSTUP_TOOLCHAIN=1.83.0 cargo llvm-cov \
  --workspace --all-targets --all-features --locked --offline \
  --json --output-path target/coverage/coverage.json \
  -- --test-threads=1
python3 tools/coverage/check_thresholds.py target/coverage/coverage.json
```

Expected: both commands exit zero; the JSON artifact contains production
coverage and the checker prints workspace line coverage at least 80%.

- [ ] **Step 4: Enforce Core and Router line coverage**

Run:

```bash
RUSTUP_TOOLCHAIN=1.83.0 cargo llvm-cov \
  -p magic-market-core -p magic-market-router \
  --all-targets --all-features --locked --offline \
  --fail-under-lines 95 -- --test-threads=1
```

Expected: exit zero with the provider-neutral data contracts and routing at
least 95% line coverage. If this gate fails, stop Slice 0 and return to Gate B;
do not lower the threshold.

## Task 8: Run bounded real live and load probes

**Files:**

- No planned file changes
- Store terminal output outside the repository for PR evidence

- [ ] **Step 1: Run every admitted live probe**

Run:

```bash
cargo run -p magic-eastmoney-rs --example live_probe --release --locked --offline
cargo run -p magic-cninfo-rs --example live_probe --release --locked --offline
cargo run -p magic-ths-rs --example live_probe --release --locked --offline
cargo run -p magic-cls-rs --example live_probe --release --locked --offline
cargo run -p magic-baidu-rs --example live_probe --release --locked --offline
```

Expected: all five commands exit zero, print non-empty records for every
advertised family, and print complete provenance/quality. Eastmoney may print
explicit failure for unadmitted fund-flow diagnostics without failing the
overall probe.

- [ ] **Step 2: Verify iWencai fails truthfully without authorization**

Run:

```bash
env -u MAGIC_IWENCAI_API_KEY -u IWENCAI_API_KEY \
  cargo run -p magic-iwencai-rs --example live_probe --release --locked --offline
```

Expected: non-zero exit with a redacted typed Authentication error and no
semantic-search success claim.

- [ ] **Step 3: Run conservative load probes**

Run:

```bash
MAGIC_EASTMONEY_LOAD_REQUESTS=5 \
MAGIC_EASTMONEY_LOAD_CONCURRENCY=1 \
MAGIC_EASTMONEY_LOAD_PACING_MS=1000 \
  cargo run -p magic-eastmoney-rs --example load_probe --release --locked --offline

MAGIC_CNINFO_LOAD_REQUESTS=3 \
MAGIC_CNINFO_LOAD_CONCURRENCY=1 \
MAGIC_CNINFO_LOAD_PACING_MS=1000 \
  cargo run -p magic-cninfo-rs --example load_probe --release --locked --offline

MAGIC_THS_LOAD_REQUESTS=3 \
MAGIC_THS_LOAD_CONCURRENCY=1 \
MAGIC_THS_LOAD_PACING_MS=1000 \
  cargo run -p magic-ths-rs --example load_probe --release --locked --offline

MAGIC_CLS_LOAD_REQUESTS=2 \
  cargo run -p magic-cls-rs --example load_probe --release --locked --offline

MAGIC_BAIDU_LOAD_REQUESTS=2 \
  cargo run -p magic-baidu-rs --example load_probe --release --locked --offline
```

Expected: every command exits zero, concurrency is one, minimum start gap is at
least 1,000 ms where the source contract requires it, and failures are zero.
Do not run or claim an iWencai load result without an authorized key.

- [ ] **Step 4: Recheck capability declarations after live evidence**

Run:

```bash
cargo test -p magic-eastmoney-rs -p magic-cninfo-rs -p magic-ths-rs \
  -p magic-cls-rs -p magic-baidu-rs -p magic-iwencai-rs \
  --test capabilities --locked --offline
```

Expected: all capability tests pass; live failures never turn an unadmitted
capability to true.

## Task 9: Build the clean release package

**Files:**

- Generated output only: `target/dist/`

- [ ] **Step 1: Confirm no tracked change remains**

Run:

```bash
git diff --quiet
git diff --cached --quiet
git status --short
```

Expected: both diff checks exit zero and status shows only the preserved user
handoff document.

- [ ] **Step 2: Build the package**

Run:

```bash
bash tools/release/package.sh
```

Expected: exit zero and print `release package:` followed by a directory under
`target/dist/` containing the twelve new probe binaries, integration docs,
licenses, lockfile, toolchain versions and `SHA256SUMS`.

- [ ] **Step 3: Verify package identity**

Run:

```bash
revision=$(git rev-parse HEAD)
test -s "target/dist/$revision/RELEASE_REVISION"
test -s "target/dist/$revision/SHA256SUMS"
test "$(sed -n '1p' "target/dist/$revision/RELEASE_REVISION")" = "$revision"
```

Expected: exit zero; the package revision equals current HEAD.

## Task 10: Independent review, PR and merge gate

**Files:**

- No planned source changes unless review finds a defect

- [ ] **Step 1: Review the complete Slice 0 diff independently**

Run:

```bash
git diff --stat 21787ed...HEAD
git diff --check 21787ed...HEAD
git log --oneline 21787ed..HEAD
```

Expected: five scoped commits—Core/Router, registered business rules,
Providers, integration documentation and release governance—with no whitespace
error.

The reviewer must independently rerun focused tests, preflight, compliance,
capability checks and at least one live probe from each public Provider family.
Any Critical or Important finding blocks the PR.

- [ ] **Step 2: Push the implementation branch**

Run:

```bash
git push -u origin feat/public-intelligence-v0.2
```

Expected: remote branch created successfully.

- [ ] **Step 3: Create the complete PR evidence body**

Create `/private/tmp/magic-market-slice0-pr-body.md` with `apply_patch` using
this exact content:

```markdown
### Refs
- overall design: `stock_analysis@8fe06b0` — `docs/superpowers/specs/2026-07-23-magic-market-data-unified-gateway-design.md`
- upstream design: `docs/superpowers/plans/2026-07-23-public-intelligence-providers.md`
- implementation baseline: `magic-market-data-rs@21787ed..feat/public-intelligence-v0.2`

### Data-Redlines
- [2.1] Providers use only real public endpoints in production; fixtures are test-only.
- [2.2] Missing optional source fields remain `None`; required fields fail explicitly.
- [2.3] Checked constructors reject invalid price, ratio, date, identity and duplicate data.
- [2.4] Source time and observation time are distinct provenance fields; stale data is never relabelled.
- [2.7] Every admitted batch records provider, observed time and batch identity.
- [2.8] All Provider methods perform transport and normalization; no logging-only implementation is admitted.
- [2.10] Pagination, capability admission, bounds, pacing and duplicate policy are registered as BR-002 and BR-009 through BR-011.

### OldModules
| module | adopt/reject | reason |
| --- | --- | --- |
| `magic-market-core` | adopt | provider-neutral checked contracts and evidence remain the canonical boundary |
| `magic-market-router` | adopt | whole-batch routing remains provider-neutral |
| `magic-tdx-rs` / `magic-tencent-rs` / `magic-sina-rs` | adopt unchanged | existing market-data Providers are outside Slice 0 |
| downstream direct HTTP acquisition | reject for future slices | downstream must consume a reviewed Magic Provider baseline |

### Threshold-Proof
- No trading, position, risk or push threshold changes.
- Public endpoint row bounds and pacing intervals are source-contract limits documented in each integration guide and exercised by deterministic/load tests.

### Business-Rules
- `BR-002` strict atomic pagination
- `BR-009` public-provider capability admission
- `BR-010` public-provider request bounds and pacing
- `BR-011` public-provider duplicate identity

### Capability gaps
- Eastmoney `fund_flow_series=false` and `post_close_flow=false`.
- iWencai `semantic_search=false` without authorized admission evidence.
- CLS `instrument_news=false`.
- No capability is promoted by fallback, inference or a failed probe.

### Validation
- Rust 1.83 release preflight: PASS
- `cargo fmt --check`: PASS
- strict workspace Clippy: PASS
- workspace tests and rustdoc: PASS
- compliance and docs links: PASS
- workspace line coverage >= 80%: PASS
- Core/Router line coverage >= 95%: PASS
- admitted live and bounded load probes: PASS; terminal evidence attached
- unauthenticated iWencai negative probe: typed `Authentication`
- release package identity and SHA256 manifest: PASS

### Rollback
1. Before merge, close this PR and keep downstream pinned to the previous reviewed Magic revision.
2. After merge, use GitHub's **Revert** action on this PR to create a revert PR.
3. Run `bash tools/release/preflight.sh` and `bash tools/release/package.sh` on the revert PR.
4. Merge the revert only after required checks pass; do not repoint downstream until a replacement baseline is reviewed.

### Merge checklist
- [x] Gate A design and business rules are reviewable.
- [x] Gate B implementation and explicit failure tests pass.
- [x] Gate C compliance, formatting, Clippy, tests and rustdoc pass.
- [x] Gate D coverage, live/load evidence and release package pass.
- [x] No production mock, silent missing-data fill or unadvertised capability remains.
- [x] Independent review has no unresolved Critical or Important finding.
```

Expected: the file contains every required evidence heading and no secret,
credential, raw response or unverified success claim.

- [ ] **Step 4: Create the draft PR with complete evidence**

Run:

```bash
gh pr create \
  --draft \
  --base main \
  --head feat/public-intelligence-v0.2 \
  --title "feat: release public intelligence providers v0.2.0" \
  --body-file /private/tmp/magic-market-slice0-pr-body.md
```

Expected: GitHub returns the draft PR URL and its body already contains all
required evidence fields.

- [ ] **Step 5: Mark ready only after all Gate evidence is attached**

Run:

```bash
gh pr ready
gh pr checks --watch
```

Expected: all required checks pass. Do not merge while any live, coverage,
review or capability-admission evidence is missing.

- [ ] **Step 6: Merge and record the immutable baseline**

Run:

```bash
gh pr merge --merge
git switch main
git pull --ff-only
git rev-parse HEAD
```

Expected: main advances to the reviewed merge commit. Record that exact SHA as
the only allowed upstream baseline in the future Slice 1 design; do not point
`stock_analysis` at the earlier dirty path state.

## Slice 0 completion checklist

- [ ] All workspace crates are `0.2.0`.
- [ ] Six Provider crates are tracked and committed.
- [ ] Core and Router remain provider-neutral.
- [ ] Every advertised family has deterministic and real evidence.
- [ ] Unverified capabilities remain false or explicitly unsupported.
- [ ] Rust 1.83 preflight, compliance, docs, coverage and packaging pass.
- [ ] Independent review has no unresolved Critical or Important finding.
- [ ] Draft PR evidence is complete and checks pass.
- [ ] The merged upstream SHA is recorded for Slice 1.
- [ ] The user's untracked stock-analysis handoff document remains untouched.
