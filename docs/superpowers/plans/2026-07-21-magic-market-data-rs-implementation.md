# magic-market-data-rs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a standalone, pure-Rust virtual workspace containing `magic-market-core` and a complete, strict, performance-verified `magic-tdx-rs` implementation derived from pinned `tdxrs` commit `18b05ffc9d8a257b5ba5add8a2d1ab038261747d`.

**Architecture:** The root is a non-publishable virtual workspace. `magic-market-core` owns provider-neutral checked values, models, provenance, quality contracts, and capability traits; `magic-tdx-rs` owns TDX source records, codec, transport, four explicit client strategies, services, local readers, and the core adapter. `stock_analysis` remains an external downstream repository and is never read or modified by these plans.

**Tech Stack:** Rust 1.83, Cargo resolver 2, Serde, thiserror, Tokio, flate2, encoding_rs, regex, tracing, Criterion, proptest, cargo-fuzz, cargo-llvm-cov, cargo-semver-checks, cargo-deny, shell compliance scripts, GitHub Actions.

---

## Why this is a plan set

The approved specification covers five independently testable subsystems. A single giant checklist would create tasks too large to review and would mix failure domains. Execute these plans in order; do not begin a later plan until the earlier plan's exit gate and commit are complete:

1. [Phase 1: workspace and core contracts](2026-07-21-magic-market-data-rs-phase-1-foundation.md)
2. [Phase 2: protocol, adjustment, and local readers](2026-07-21-magic-market-data-rs-phase-2-protocol.md)
3. [Phase 3: transport and four client strategies](2026-07-21-magic-market-data-rs-phase-3-clients.md)
4. [Phase 4: complete services, facade, and core adapter](2026-07-21-magic-market-data-rs-phase-4-services.md)
5. [Phase 5: compatibility, performance, documentation, and release evidence](2026-07-21-magic-market-data-rs-phase-5-release.md)

The downstream `stock_analysis` migration is deliberately absent. It needs a separate design and implementation plan in that repository after this workspace produces a fixed version or full Git revision.

## Locked source and scope

- Upstream repository: `https://github.com/jiangtaovan/tdxrs`
- Upstream commit: `18b05ffc9d8a257b5ba5add8a2d1ab038261747d`
- Upstream package version: `0.6.7`
- License: MIT; preserve notice in `LICENSES/tdxrs-MIT.txt` and per-file provenance inventory.
- Include: all pure-Rust protocol, adjustment, reader, sync/direct/async/smart client, finance, fund, block, and F10/profile behavior.
- Exclude: PyO3, Python modules, CLI, DataFrame helpers, and downloader conveniences.
- Intentional differences: no silent truncation, missing-as-zero, incomplete adjustment, partial pagination, empty-success retry exhaustion, public panic, or swallowed task error.

## Repository file map

```text
magic-market-data-rs/
├── AGENTS.md
├── Cargo.toml
├── Cargo.lock
├── LICENSE-APACHE
├── LICENSE-MIT
├── LICENSES/tdxrs-MIT.txt
├── README.md
├── README.en.md
├── rust-toolchain.toml
├── deny.toml
├── .github/workflows/{ci,msrv,security,bench}.yml
├── crates/
│   ├── magic-market-core/
│   │   ├── Cargo.toml
│   │   ├── README.md
│   │   ├── src/{lib,error,instrument,value,time,model,request,provenance,batch,quality,provider}.rs
│   │   └── tests/{values,quality,providers}.rs
│   └── magic-tdx-rs/
│       ├── Cargo.toml
│       ├── README.md
│       ├── src/
│       │   ├── {lib,prelude,error,config,adapter}.rs
│       │   ├── source/{mod,enums,types}.rs
│       │   ├── codec/{mod,cursor,varint,decompress}.rs
│       │   ├── protocol/{mod,header,packet}.rs
│       │   ├── protocol/parsers/{mod,bars,quotes,instruments,minute,trades,finance,xdxr,block}.rs
│       │   ├── adjustment/{mod,factor,service}.rs
│       │   ├── reader/{mod,daily,min,financial,block,profile}.rs
│       │   ├── transport/{mod,endpoint,blocking,asynchronous,pool,response}.rs
│       │   ├── rate_limit/{mod,clock,limiter}.rs
│       │   ├── client/{mod,blocking,direct,asynchronous,smart}.rs
│       │   └── service/{mod,common,bars,quotes,instruments,minute,trades,finance,actions,funds,blocks,profile}.rs
│       ├── tests/{golden,differential,loopback,strict_failures,capability_matrix}.rs
│       ├── tests/fixtures/{manifest.json,protocol,readers}/
│       ├── benches/{codec,reader,loopback_clients}.rs
│       ├── examples/{blocking_bars,async_quotes,direct_reader}.rs
│       └── fuzz/fuzz_targets/{decode_response,parse_bars,parse_quotes,readers}.rs
├── docs/{ENGINEERING_RULES,business_rules,ARCHITECTURE,API_GUIDE,DATA_MODEL,PROTOCOL,ERROR_HANDLING,CLIENTS_AND_CONCURRENCY,RATE_LIMITING,COMPATIBILITY,MIGRATION_FROM_TDXRS,UPSTREAM,PERFORMANCE,TESTING,OPERATIONS,SECURITY,CONTRIBUTING,CHANGELOG,SUPPORT}.md
├── provenance/{upstream-files.toml,pure-rust.patch}
├── tools/{compliance/check.sh,coverage/check_thresholds.py,upstream/fetch.sh,upstream/verify.sh,bench/compare.py,docs/check_links.sh}
└── artifacts/{compatibility,benchmarks}/.gitkeep
```

## Cross-plan invariants

- Every public fallible operation returns a typed error with operation and available field/server/request context.
- `source_at: None` remains unknown; `fetched_at` never substitutes for it.
- Strict methods are atomic across pages/batches/adjustment context. Explicit best-effort methods use different names and return structured incompleteness.
- Request limits, ordering, deduplication, server selection, queueing, retry, and rate policies are registered in `docs/business_rules.md` before code.
- Default tests use local fixtures or loopback only. Live public-server checks are opt-in diagnostics and never claim success when skipped.
- No code from `/private/tmp` is a build dependency. The pinned checkout may be inspected during implementation, but committed provenance and reproducible fetch scripts are the source of record.
- No production Cargo manifest contains a path dependency on `stock_analysis` or any sibling checkout.
- Each task stages only its listed files and ends with `git diff --cached --check` before commit.

## Global validation commands

Run after every phase, adding narrower commands earlier as each crate becomes available:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo test --workspace --doc --all-features
cargo doc --workspace --all-features --no-deps
bash tools/compliance/check.sh
```

Expected: every command exits `0`; Clippy emits no warnings; default tests make no external network connection.

The release phase additionally runs:

```bash
cargo +1.83.0 check --workspace --all-targets
cargo llvm-cov --workspace --all-features --json --output-path target/coverage/coverage.json -- --test-threads=1
python3 tools/coverage/check_thresholds.py target/coverage/coverage.json
cargo semver-checks check-release -p magic-market-core
cargo semver-checks check-release -p magic-tdx-rs
cargo deny check
```

Expected: MSRV build passes; overall line coverage is at least 80%; configured core files are at least 95%; SemVer, license, advisory, ban, and source checks pass.

## Execution checkpoints

### Task 1: Execute Phase 1

- [ ] Complete every checkbox in the Phase 1 plan.
- [ ] Verify the Phase 1 exit gate.
- [ ] Record exact commands and commit SHA in `.planning/2026-07-21-magic-tdx-rs/progress.md`.
- [ ] Stop for review before Phase 2.

### Task 2: Execute Phase 2

- [ ] Complete every checkbox in the Phase 2 plan.
- [ ] Verify golden, strict-failure, differential, reader, and adjustment evidence.
- [ ] Record exact commands and commit SHA in the active progress file.
- [ ] Stop for review before Phase 3.

### Task 3: Execute Phase 3

- [ ] Complete every checkbox in the Phase 3 plan.
- [ ] Verify loopback correctness, cancellation, backpressure, timeout, pool, retry, and rate-limit evidence for all four client types.
- [ ] Record exact commands and commit SHA in the active progress file.
- [ ] Stop for review before Phase 4.

### Task 4: Execute Phase 4

- [ ] Complete every checkbox in the Phase 4 plan.
- [ ] Verify every pinned-upstream pure-Rust operation has an `Adopt`, `Replaced`, or `Intentional Difference` row and executable test.
- [ ] Record exact commands and commit SHA in the active progress file.
- [ ] Stop for review before Phase 5.

### Task 5: Execute Phase 5

- [ ] Complete every checkbox in the Phase 5 plan.
- [ ] Run all Gate B/C/D commands and preserve raw machine-readable artifacts.
- [ ] Report `In Progress / Blocked` unless every applicable library-level acceptance item has evidence.
- [ ] Do not start or claim the external `stock_analysis` migration.

## Rollback

Each phase uses independently revertible commits. If a phase fails for an architectural reason, stop and return to the approved design rather than patching around the contract. The library rollback command is:

```bash
PHASE_COMMIT=$(git rev-parse HEAD)
git revert "$PHASE_COMMIT"
cargo test --workspace --all-targets --all-features
```

Expected: the reverted workspace passes the last completed phase's gate.
