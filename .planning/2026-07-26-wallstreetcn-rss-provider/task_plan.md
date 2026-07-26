# Task Plan: WallstreetCN RSS News Provider

## Goal

Add a first-class, bounded, metadata-only WallstreetCN RSS Provider without
returning, storing, indexing, or redistributing descriptions or article
bodies.

## Current Phase

Complete

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
- [x] Add bounded live and load probes.
- **Status:** complete

### Phase 3: Admission and Release Registration

- [x] Run production-client admission probes and set the truthful capability.
- [x] Register README, deployment, integration, business rule, upstream,
  compliance, and packaging documentation.
- **Status:** complete

### Phase 4: Gates and Review

- [x] Pass strict coverage and all release gates.
- [x] Complete independent code review and final handoff.
- **Status:** complete

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
| Optional title-match probe hit one DNS resolution failure | First title-match attempt | Preserved the typed `Transport` error and reran the same bounded release probe outside the sandbox; it passed without weakening any contract. |
| Cargo warned that several Provider examples share `live_probe` and `load_probe` output names | Full preflight | Preserved the workspace-wide warning because it does not affect correctness or release packaging; the package script gives Provider probes distinct artifact names. |
| Independent review found that ignored XML content and declarations were not fully strict | Final review | Added adversarial red tests, document-wide XML 1.0 character validation, checked comments and attributes, unique ordered XML 1.0 declaration validation, and skipped decoding ignored text before rerunning live/load, coverage, and full preflight gates. |
| Follow-up review found that `U+FFFE` / `U+FFFF` references and normalized declaration values crossed the XML boundary | Review remediation | Added red tests, applied the XML 1.0 predicate to numeric references, and compared raw UTF-8 declaration values before rerunning live/load, coverage, and full preflight gates. |
| One focused Cargo test command used two filter arguments | Second remediation red phase | Preserved Cargo's explicit `unexpected argument` failure and reran each filter separately. |
| Strict crate Clippy found one `needless_borrow` | Second remediation verification | Removed the unnecessary borrow and reran strict crate Clippy successfully. |

## Notes

- Worktree:
  `/Users/zhangzhen/Desktop/Quant/magic-market-data-rs/.worktrees/yonhap-news-provider`
- Branch: `feat/yonhap-news-provider`
- Approved design commit: `c2f6348`
- Implementation plan commit: `0f63041`
