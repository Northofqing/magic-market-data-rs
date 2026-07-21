# magic-market-data-rs / magic-tdx-rs task plan

## Goal

Design the umbrella project `magic-market-data-rs` and build its first complete, stable, pure-Rust source driver `magic-tdx-rs`, based on auditable upstream behavior, with standard-purpose APIs and throughput/concurrency parity with `jiangtaovan/tdxrs`.

## Constraints

- Follow Gate A -> B -> C -> D in `AGENTS.md`.
- Do not modify implementation code before the brainstorming design and written spec are approved.
- Preserve unrelated dirty-worktree changes.
- Keep production failures explicit; never substitute fake, zero, empty, stale, or silently downgraded data.
- Preserve upstream licensing and source provenance.
- Documentation is a first-class deliverable: repository/crate README, rustdoc, architecture, protocol notes, API reference, error catalog, data semantics, concurrency/rate-limit guide, benchmark methodology/results, compatibility matrix, migration guide, examples, provenance/license notices, changelog, and contribution guide.
- Documentation examples must compile or run as doctests where practical; documentation links and public API docs are part of validation.

## Phases

### Phase 1: Explore project and upstream context

Status: complete

- Inspect local provider/API/test/benchmark conventions and recent commits.
- Inspect the complete upstream tree, documentation, public API, tests, benchmarks, and license at a fixed commit.
- Inspect comparable Rust TDX implementations only where upstream behavior is ambiguous.

### Phase 2: Clarify requirements

Status: complete

- Ask one question at a time about packaging, compatibility, and acceptance criteria.

### Phase 3: Compare approaches

Status: complete

- Present 2-3 integration/extraction approaches and recommend one.

### Phase 4: Present and approve design

Status: complete

- Cover architecture, API surface, data flow, error handling, concurrency/rate limiting, provenance, tests, benchmarks, rollback, and old-module relations.

### Phase 5: Write and review Gate A spec

Status: in_progress

- Write `docs/superpowers/specs/2026-07-21-magic-tdx-rs-design.md`.
- Self-review for placeholders, contradictions, scope, ambiguous acceptance criteria, and unsupported code claims.
- Commit the design document alone and ask the user to review it.
- The design and its planning artifacts were moved byte-for-byte from the adjacent
  `stock_analysis` repository into the dedicated `magic-market-data-rs` repository
  on 2026-07-21.
- Written review must explicitly resolve whether the physical repository move also
  replaces the approved in-repository, non-virtual workspace layout with a standalone
  workspace and downstream `stock_analysis` integration.

### Phase 6: Write implementation plan

Status: pending

- After written-spec approval, use the `writing-plans` skill.

### Phase 7: Implement Gate B

Status: pending

- Execute only the approved implementation plan.
- Add failure-path tests and upstream compatibility evidence.

### Phase 8: Verify Gates C and D

Status: pending

- Run formatting, strict Clippy, tests, compliance, coverage, benchmarks, and controlled live-data validation.
- Run rustdoc/doctests, compile documented examples, and verify documentation links/API coverage.
- Report status as In Progress/Blocked unless every applicable DoD item has evidence.

## Decisions

- No implementation edits before design approval.
- Planning artifacts use this task-specific directory without changing `.planning/.active_plan`.
- Umbrella project/repository name: `magic-market-data-rs`.
- TDX source-driver crate name: `magic-tdx-rs`.
- The driver remains independently reusable and has no dependency on `stock_analysis`.
- Initial delivery scope: `magic-market-core + magic-tdx-rs` only. Multi-provider orchestration is deferred to a subsequent design cycle.
- API strategy: comprehensive functional/result parity with pinned upstream, exposed through a new idiomatic Rust-first stable facade. Upstream internal module-path/source compatibility is not a requirement.
- Numeric strategy: dual-layer model. TDX protocol-facing decoded records preserve upstream `f64`/source fields for parity and speed; normalized `magic-market-core` types use checked strong types such as `Price`, `Quantity`, and `Money`.
- Client strategy: blocking pool, Tokio async pool, stateless Direct, and Smart policy clients remain distinct first-class APIs with shared protocol/parsing/capability contracts.
- v1 functional scope: all pinned-upstream pure Rust capabilities (equity/index quotes and bars, minute/tick/history, security list/count, finance, XDXR, funds, blocks, F10/profile, local readers, and all four client strategies). Python CLI, DataFrame helpers, and downloader are out of scope.
- Reliability policy: strict by default. Malformed/truncated packets, missing fields, over-limit batches, incomplete pagination/context, adjustment-source failure, and exhausted empty responses are typed errors. Chunking or best-effort behavior requires explicit opt-in and visible outcome metadata.
- Performance policy: compare against pinned upstream in same-machine, same-profile, same-fixture/server alternating A/B runs. Reader/protocol and 1/5/60-concurrency client throughput regressions are capped at 5%; controlled live-network median/p95 regression is capped at 10%, with no lower success rate.
- Platform policy: MSRV Rust 1.83; Linux, macOS, and Windows; x86_64 and aarch64. Cross-platform CI proves build/test portability while fixed benchmark hosts provide comparable performance evidence.
- Repository organization: convert the existing root package into a non-virtual Cargo workspace containing the root application, `magic-market-core`, and `magic-tdx-rs`, with full-gate commands covering all members.
- Implementation approach: audited extraction and hardening from pinned upstream. Preserve verified protocol/performance-critical logic, remove Python coupling, repair reliability gaps, add a Rust-first facade, and prove compatibility through differential tests and benchmarks.
- Design section 1 approved: layered workspace boundary with `magic-market-core`, isolated `magic-tdx-rs` protocol/transport/service/adapter layers, and a thin existing-project adapter retaining project-specific policy.
- Design section 2 approved: four explicit client types, typed builders/requests, TDX source records plus normalized provider results, capability traits, provenance-bearing batches, and a documented SemVer-stable facade.
- Design section 3 approved: bounds-checked parsing, typed contextual errors, strict full-batch/adjustment semantics, explicit no-data/empty distinction, observable optional policies, and no public panics or swallowed task errors.
- Design section 4 approved: distinct lock-safe client execution models, bounded backpressure/timeouts/retry budgets, explicit adaptive rate-limit scopes, and three-layer reproducible performance gates.
- Design section 5 approved: unit/golden/property/fuzz/protocol-replay/integration/live-diagnostic test layers, pinned-upstream differential evidence, cross-platform MSRV CI, coverage and SemVer gates.
- Design section 6 approved: complete landed documentation set, enforced public rustdoc/examples/link checks, evidence-backed protocol/performance/compatibility claims, complete Chinese technical docs with English README/API synopsis, and pre-1.0 stabilization.
- Design section 7 approved: additive staged migration, preservation of application policy, no silent production fallback, explicit old-module disposition, registered business rules, independently revertible integration, and all Gate A-D evidence before release.
- Repository relocation: all four design/planning artifacts now live in the dedicated
  `magic-market-data-rs` repository; the formal design remains byte-identical pending
  written review of the workspace-boundary consequence.

## Errors encountered

| Error | Attempt | Resolution |
| --- | --- | --- |
| Sandboxed `git ls-remote` could not resolve GitHub | 1 | Retried with approved network escalation; upstream commit pinned successfully. |
| Upstream all-feature tests fail at PyO3 dylib linking on macOS x86_64 | 1 | Treat as proof for pure-Rust architectural separation; do not retry unchanged. |
| Multi-hunk phase-status patch did not match its context | 1 | Re-read the task plan and applied the exact smaller hunks. |
| New design document is hidden by repository `/docs` ignore rule | 1 | Force-add only the exact design file and verify the staged manifest before commit. |
| Destination Git index is read-only inside the workspace sandbox | 1 | Retried the exact scoped `git add` with approved elevated Git metadata access. |
