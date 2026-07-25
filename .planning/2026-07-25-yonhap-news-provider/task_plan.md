# Task Plan: Yonhap Chinese RSS News Provider

## Goal

Add a provenance-preserving Yonhap Chinese RSS Provider that exposes bounded
news metadata without fetching, storing, or redistributing article bodies.

## Current Phase

Phase 1

## Phases

### Phase 1: Design and Baseline

- [x] Inspect existing news contracts, Provider identities, Router adapters,
  release gates, and representative Provider crates.
- [x] Verify the official Yonhap Chinese RSS directory and copyright boundary.
- [x] Present alternatives and receive approval for metadata-only Chinese RSS.
- [x] Create an isolated worktree and pass the workspace baseline.
- [x] Write and self-review the approved design.
- [ ] Commit the approved design.
- [ ] Obtain user review of the written specification.
- **Status:** in_progress

### Phase 2: Detailed Implementation Plan

- [ ] Map exact files and dependency changes.
- [ ] Write a TDD-oriented implementation plan.
- [ ] Self-review the plan against the approved specification.
- [ ] Obtain execution approval.
- **Status:** pending

### Phase 3: Provider Implementation

- [ ] Add the Yonhap Provider identity and core identity tests.
- [ ] Add the standalone bounded RSS Provider crate.
- [ ] Add deterministic parser, transport, pacing, and failure tests.
- [ ] Add Router identity coverage and capability tests.
- [ ] Add live and load probes.
- **Status:** pending

### Phase 4: Admission, Documentation, and Release Gates

- [ ] Run the bounded official live probe.
- [ ] Advertise only the capability proven by the live result.
- [ ] Register workspace, compliance, deployment, and integration docs.
- [ ] Run formatting, tests, Clippy, compliance, docs, coverage, and release
  checks.
- [ ] Obtain final code review and integrate without touching user-owned files.
- **Status:** pending

## Decisions

| Decision | Rationale |
| --- | --- |
| Use a standalone `magic-yonhap-rs` crate | Yonhap needs a first-class Provider identity and independent provenance. |
| Use the official simplified-Chinese RSS feeds | The user selected Chinese metadata and Yonhap explicitly documents these feeds for RSS readers. |
| Never map RSS descriptions into summary or content | Yonhap's terms prohibit unauthorized copying, storage, and redistribution of its articles. |
| Support all seven official RSS channels | The source publishes a closed, documented channel set; `Economy` directly serves the initiating financial-news use case. |
| Keep keyword matching in the probe only | Core currently defines bounded latest news, not historical search; a new search contract is outside this feature. |
| Gate public capability on bounded live evidence | Deterministic fixtures prove parsing, while a live probe proves the current production path. |

## Errors Encountered

| Error | Attempt | Resolution |
| --- | --- | --- |
| Direct `curl` TLS handshake to `cn.yna.co.kr` returned `SSL_ERROR_SYSCALL` inside and outside the sandbox | 2 | Preserve the failure as current evidence; test the production Rust TLS path during implementation and keep capability false if live admission cannot be proved. |

## Notes

- Primary checkout user-owned changes remain untouched.
- Worktree:
  `/Users/zhangzhen/Desktop/Quant/magic-market-data-rs/.worktrees/yonhap-news-provider`
- Branch: `feat/yonhap-news-provider`
- Re-read this plan before capability or copyright decisions.
