# Task Plan: Official Macro, Global Data, SEC, and Financial News

## Goal

Add first-class, source-attributed Providers for official Chinese macro and
money-market data, public global macro data, SEC filing metadata, and bounded
financial-news metadata without weakening authorization, evidence, or release
gates.

## Current Phase

Release gates and final integration

## Phases

### Phase 1: Design and Source Boundaries

- [x] Confirm the first-batch source scope and deferred paid sources.
- [x] Audit existing Core contracts and Provider identities.
- [x] Establish an isolated worktree from current `origin/main`.
- [x] Complete the written design specification.
- [x] Commit the written design specification.
- [x] Receive review approval for the written specification.
- **Status:** complete

### Phase 2: Detailed Implementation Plan

- [x] Write file-by-file, test-first implementation plans split by independent
  subsystem.
- [x] Commit the reviewed implementation plan.
- **Status:** complete

### Phase 3: Core and Provider Implementation

- [x] Add provider-neutral macro, rate, fixing, and filing contracts.
- [x] Add the bounded shared transport support required by the new Providers.
- [x] Implement NBS, PBC, CFETS, FRED, IMF, World Bank, and SEC Providers.
- [x] Implement public metadata-only Xinhua Finance, Yicai, and Securities
  Times Providers where live source audits prove an admissible endpoint.
- **Status:** complete

### Phase 4: Routing, Documentation, and Admission

- [x] Add provider-neutral Router sources and exact-identity failover rules.
- [x] Add fixtures, malformed-input tests, bounded live probes, and serial load
  probes.
- [x] Update README, integration, deployment, upstream, business-rule,
  compliance, and packaging documentation.
- **Status:** complete

### Phase 5: Release Gates and Review

- [ ] Pass formatting, all-target tests, strict Clippy, Rustdoc, documentation
  links, compliance, coverage, packaging, and release preflight.
- [ ] Complete code review and integrate only after all findings are resolved.
- **Status:** in progress

## Decisions

| Decision | Rationale |
| --- | --- |
| Use one crate per upstream source | Preserves source identity and isolates protocol changes. |
| Add provider-neutral Core contracts before Providers | Prevents source payloads from becoming downstream APIs. |
| Treat news as metadata-only | Avoids copying article bodies and keeps the existing `NewsItem` boundary. |
| Keep paid/authenticated feeds in a later phase | Public endpoints do not justify bypassing licenses or login controls. |
| Admit capabilities only after deterministic and live proof | Fixtures cannot prove a current production endpoint. |
| Split implementation into foundation, China official, global macro, SEC, news, and integration plans | Each subsystem can produce independently testable software and remain reviewable. |
| Keep World Bank production admission false under the approved mandatory-unit contract | The audited official structured unit fields are empty; inferring units from prose would violate the contract. |
| Keep PBC social financing false in this slice | Current official flow tables are PDF/XLSX rather than the admitted structured HTML family; no generic document scraper is introduced. |
| Keep CFETS DR007 false | No equivalent bounded public history contract was proven; R007/Shibor are not substitutes. |

## Errors Encountered

| Error | Attempt | Resolution |
| --- | --- | --- |
| None | Baseline and design phase | Build and all-target test baselines passed without failures. |
| Direct source-audit curl first failed DNS inside the sandbox | First page audit | Retried outside the sandbox as required. |
| NBS page returned HTTP 403 to a minimal curl client | Escalated page audit | A later bounded Rust landing probe succeeded, but no supported machine-series contract was proved; production capability remains false without browser emulation. |
| PBC live page was GBK/Excel HTML rather than the compact planning fixture | First release live probe | Added strict charset handling and an audited 19×16 bilingual grid parser; two subsequent live probes and the serial load gate passed. |
| Yicai current title had leading presentation whitespace | First post-review live probe | Trim only outer Unicode display whitespace after rejecting controls/empty values; two subsequent live probes and the serial load gate passed. |
| World Bank indicator metadata omitted page-level source fields and returned an empty row unit | Real diagnostic | Match the real indicator envelope, validate row source identity, and preserve the mandatory-unit blocker without inferring from prose. |
| First Securities Times JSON inspection assumed `data` was always an array | Quick-news audit | The source returned `data:""` for explicit empty cursors; recorded the polymorphic empty shape and changed the next request to omit undefined cursor keys exactly as jQuery does. |
| First direct CFETS page bundle stopped on HTTP 404 | CFETS audit | Inspect each official page separately instead of assuming all searched paths remain live; no alternate private/member endpoint is used. |
| A documentation audit command used an unmatched `docs/upstream*` zsh glob | Release-file mapping | Use the exact tracked file `docs/UPSTREAM.md` and explicit paths; no planning evidence was lost. |
| Parallel package verification exhausted disk space with several independent Rust target directories | Provider review checkpoint | Removed only explicitly named, reproducible temporary build caches and switched later verification to one reused target directory. |

## Notes

- Worktree:
  `/Users/zhangzhen/Desktop/Quant/magic-market-data-rs/.worktrees/official-macro-global-news`
- Branch: `feat/official-macro-global-news`
- Base: `origin/main` at `660902f`
- Plan index:
  `docs/superpowers/plans/2026-07-29-official-macro-global-news-index.md`
- Plan commit: `1e861ed`
