# Task Plan: WallstreetCN RSS News Provider

## Goal

Add a first-class, bounded, metadata-only WallstreetCN RSS Provider without
returning, storing, indexing, or redistributing descriptions or article
bodies.

## Current Phase

Phase 2

## Phases

### Phase 1: Identity and Transport

- [x] Add the Core Provider identity and provider-neutral Router evidence
  tests.
- [x] Add the standalone crate, exact request contract, bounded HTTPS
  transport, and clone-shared pacing.
- **Status:** complete

### Phase 2: Parser and Public Contract

- [x] Add strict complete-feed RSS parsing and metadata-only mapping.
- [x] Add public capability and typed failure tests.
- [ ] Add bounded live and load probes.
- **Status:** in_progress

### Phase 3: Admission and Release Registration

- [ ] Run production-client admission probes and set the truthful capability.
- [ ] Register README, deployment, integration, business rule, upstream,
  compliance, and packaging documentation.
- **Status:** pending

### Phase 4: Gates and Review

- [ ] Pass strict coverage and all release gates.
- [ ] Complete independent code review and final handoff.
- **Status:** pending

## Decisions

| Decision | Rationale |
| --- | --- |
| Use one standalone Provider crate | Preserves exact WallstreetCN provenance and Router neutrality. |
| Use only the first-party public RSS endpoint | Avoids undocumented APIs, authentication, and webpage crawling. |
| Never map RSS descriptions | The feed includes article content and the approved contract is metadata-only. |
| Gate `global_news` on production evidence | Fixtures prove behavior but cannot prove current network availability. |

## Errors Encountered

| Error | Attempt | Resolution |
| --- | --- | --- |
| Locked manifest initially required a local lock refresh | First crate test | Ran Cargo offline without `--locked` once, then restored locked verification. |
| Parser red test could not find `parse_response` | Parser TDD red phase | Implemented the strict RSS state machine and canonical mapping. |
| Chinese text was placed in a raw byte string fixture | First transport format check | Used a UTF-8 string followed by `as_bytes()`. |
| Transport-only Clippy reported parser-bound entries as dead code | Transport checkpoint | Completed the approved parser/Provider wiring instead of adding a temporary lint allowance; final crate Clippy passed. |

## Notes

- Worktree:
  `/Users/zhangzhen/Desktop/Quant/magic-market-data-rs/.worktrees/yonhap-news-provider`
- Branch: `feat/yonhap-news-provider`
- Approved design commit: `c2f6348`
- Implementation plan commit: `0f63041`
