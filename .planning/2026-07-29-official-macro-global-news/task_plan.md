# Task Plan: Official Macro, Global Data, SEC, and Financial News

## Goal

Add first-class, source-attributed Providers for official Chinese macro and
money-market data, public global macro data, SEC filing metadata, and bounded
financial-news metadata without weakening authorization, evidence, or release
gates.

## Current Phase

Design review

## Phases

### Phase 1: Design and Source Boundaries

- [x] Confirm the first-batch source scope and deferred paid sources.
- [x] Audit existing Core contracts and Provider identities.
- [x] Establish an isolated worktree from current `origin/main`.
- [x] Complete the written design specification.
- [x] Commit the written design specification.
- [ ] Receive review approval for the written specification.
- **Status:** in progress

### Phase 2: Detailed Implementation Plan

- [ ] Write a file-by-file, test-first implementation plan.
- [ ] Commit the approved implementation plan.
- **Status:** pending

### Phase 3: Core and Provider Implementation

- [ ] Add provider-neutral macro, rate, fixing, and filing contracts.
- [ ] Add the bounded shared transport support required by the new Providers.
- [ ] Implement NBS, PBC, CFETS, FRED, IMF, World Bank, and SEC Providers.
- [ ] Implement public metadata-only Xinhua Finance, Yicai, and Securities
  Times Providers where live source audits prove an admissible endpoint.
- **Status:** pending

### Phase 4: Routing, Documentation, and Admission

- [ ] Add provider-neutral Router sources and exact-identity failover rules.
- [ ] Add fixtures, malformed-input tests, bounded live probes, and serial load
  probes.
- [ ] Update README, integration, deployment, upstream, business-rule,
  compliance, and packaging documentation.
- **Status:** pending

### Phase 5: Release Gates and Review

- [ ] Pass formatting, all-target tests, strict Clippy, Rustdoc, documentation
  links, compliance, coverage, packaging, and release preflight.
- [ ] Complete code review and integrate only after all findings are resolved.
- **Status:** pending

## Decisions

| Decision | Rationale |
| --- | --- |
| Use one crate per upstream source | Preserves source identity and isolates protocol changes. |
| Add provider-neutral Core contracts before Providers | Prevents source payloads from becoming downstream APIs. |
| Treat news as metadata-only | Avoids copying article bodies and keeps the existing `NewsItem` boundary. |
| Keep paid/authenticated feeds in a later phase | Public endpoints do not justify bypassing licenses or login controls. |
| Admit capabilities only after deterministic and live proof | Fixtures cannot prove a current production endpoint. |

## Errors Encountered

| Error | Attempt | Resolution |
| --- | --- | --- |
| None | Baseline and design phase | Build and all-target test baselines passed without failures. |

## Notes

- Worktree:
  `/Users/zhangzhen/Desktop/Quant/magic-market-data-rs/.worktrees/official-macro-global-news`
- Branch: `feat/official-macro-global-news`
- Base: `origin/main` at `660902f`
