# magic-market-data-rs / magic-tdx-rs task plan

## Goal

Design the umbrella project `magic-market-data-rs` and build its first complete, stable, pure-Rust source driver `magic-tdx-rs`, based on auditable upstream behavior, with standard-purpose APIs and throughput/concurrency parity with `jiangtaovan/tdxrs`.

## Constraints

- Preserve the applicable Gate A -> B -> C -> D safety model and codify it in this
  independent repository's own `AGENTS.md` during implementation phase 1.
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

Status: complete

- Write `docs/superpowers/specs/2026-07-21-magic-tdx-rs-design.md`.
- Self-review for placeholders, contradictions, scope, ambiguous acceptance criteria, and unsupported code claims.
- Commit the design document alone and ask the user to review it.
- The design and its planning artifacts were moved byte-for-byte from the adjacent
  `stock_analysis` repository into the dedicated `magic-market-data-rs` repository
  on 2026-07-21.
- The user approved a standalone pure virtual workspace with `stock_analysis` treated
  as an external downstream consumer.
- Revised and self-reviewed the formal spec, then committed the design-only change as
  `faaff5e` (`docs: make magic market data standalone`).

### Phase 6: Write implementation plan

Status: complete

- After written-spec approval, use the `writing-plans` skill. Completed with the
  ordered five-file plan set.

### Phase 7: Implement Gate B

Status: complete

- Execute only the approved implementation plan.
- Add failure-path tests and upstream compatibility evidence.

### Phase 8: Verify Gates C and D

Status: complete

- Run formatting, strict Clippy, tests, compliance, coverage, benchmarks, and controlled live-data validation.
- Run rustdoc/doctests, compile documented examples, and verify documentation links/API coverage.
- Report status as In Progress/Blocked unless every applicable DoD item has evidence.

### Phase 9: Provider expansion and downstream P0/P1 contracts

Status: complete

- Completed and pushed the provider-neutral Quote, OrderBook, MoneyFlow,
  Auction, and Trades contracts plus TDX/EMQuant dispositions.
- Completed TDX current/historical trade pagination with unit and live
  cross-page evidence.
- Packaged the EMQuant bridge and SDK runtime under ignored project paths;
  activation is complete and live records await account API entitlement.
- Completed: represent Beijing exchange and auditable security metadata,
  including explicit unavailable fields instead of inferred fake completeness.
- Completed final tests, strict Clippy, rustdoc/doctests, Rust 1.83 check,
  documentation links, compliance, TDX live validation, and EMQuant failure-path
  validation. EMQuant live records remain externally blocked by account
  entitlement `10001003/EQERR_NO_ACCESS`, not unfinished repository code.

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
- Repository organization: a standalone pure virtual Cargo workspace with exactly two
  independently versioned library members, `magic-market-core` and `magic-tdx-rs`.
  The root has no package or umbrella facade crate and commits `Cargo.lock` for
  reproducible tests and benchmarks.
- Downstream organization: `stock_analysis` consumes a fixed published version or full
  Git revision and runs its own integration, freshness, compliance, production, and
  audit Gates; production path dependencies are prohibited.
- Implementation approach: audited extraction and hardening from pinned upstream. Preserve verified protocol/performance-critical logic, remove Python coupling, repair reliability gaps, add a Rust-first facade, and prove compatibility through differential tests and benchmarks.
- Design section 1 approved: layered workspace boundary with `magic-market-core`, isolated `magic-tdx-rs` protocol/transport/service/adapter layers, and a thin existing-project adapter retaining project-specific policy.
- Design section 2 approved: four explicit client types, typed builders/requests, TDX source records plus normalized provider results, capability traits, provenance-bearing batches, and a documented SemVer-stable facade.
- Design section 3 approved: bounds-checked parsing, typed contextual errors, strict full-batch/adjustment semantics, explicit no-data/empty distinction, observable optional policies, and no public panics or swallowed task errors.
- Design section 4 approved: distinct lock-safe client execution models, bounded backpressure/timeouts/retry budgets, explicit adaptive rate-limit scopes, and three-layer reproducible performance gates.
- Design section 5 approved: unit/golden/property/fuzz/protocol-replay/integration/live-diagnostic test layers, pinned-upstream differential evidence, cross-platform MSRV CI, coverage and SemVer gates.
- Design section 6 approved: complete landed documentation set, enforced public rustdoc/examples/link checks, evidence-backed protocol/performance/compatibility claims, complete Chinese technical docs with English README/API synopsis, and pre-1.0 stabilization.
- Design section 7 approved: additive staged migration, preservation of application policy, no silent production fallback, explicit old-module disposition, registered business rules, independently revertible integration, and all Gate A-D evidence before release.
- Repository relocation: all design/planning artifacts live in the dedicated
  `magic-market-data-rs` repository, and the formal design now explicitly defines its
  standalone virtual-workspace and downstream-consumer boundaries.

## Errors encountered

| Error | Attempt | Resolution |
| --- | --- | --- |
| Sandboxed `git ls-remote` could not resolve GitHub | 1 | Retried with approved network escalation; upstream commit pinned successfully. |
| Upstream all-feature tests fail at PyO3 dylib linking on macOS x86_64 | 1 | Treat as proof for pure-Rust architectural separation; do not retry unchanged. |
| Multi-hunk phase-status patch did not match its context | 1 | Re-read the task plan and applied the exact smaller hunks. |
| New design document is hidden by repository `/docs` ignore rule | 1 | Force-add only the exact design file and verify the staged manifest before commit. |
| Parallel EMQuant fake bridges intermittently produced empty JSON | 1 | Reproduced both tests individually, identified timestamp/PID temp-path collision, and added an atomic per-process id. |
| Destination Git index is read-only inside the workspace sandbox | 2 | Retried exact scoped `git add` operations with explicit approved elevated Git metadata access. |
| Source cleanup pathspec did not match after its active branch changed | 1 | Re-read branch/index status; the active source branch does not track the spec, so no cleanup commit is required. Preserved the historical design branch for recovery. |
| Final range check used nonexistent `HEAD~2` in a two-commit repository | 1 | Validate the root and follow-up commits individually with `git diff-tree --check --root`. |
| Sandboxed crates.io API lookup failed DNS resolution; escalated retry returned HTTP 403 | 2 | Do not repeat the API call. Keep release tools reproducible through a committed tool-version manifest resolved and verified during implementation, without claiming an unverified current version in this plan. |
| Multi-file implementation-plan cleanup patch missed the exact Phase 2 enum sentence | 1 | No file changed; inspect exact contexts and apply smaller scoped patches. |
