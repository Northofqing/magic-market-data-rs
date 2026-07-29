# Official Macro, Global Data, SEC, and Financial News Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use executing-plans to implement each
> linked plan task-by-task with the listed red/green checkpoints.

**Goal:** Deliver checked provider-neutral contracts, bounded transport, ten
source-aligned Providers, evidence-preserving routing, truthful capability
admission, and release documentation for the approved official macro, SEC, and
metadata-only financial-news scope.

**Architecture:** Work proceeds through six independently committable plans.
Core and transport land first. Source Providers depend only on Core and the
transport support crate. Router stays provider-neutral. Source-specific live
admission happens only after deterministic tests pass and two consecutive
bounded production fetches plus the serial load probe succeed.

**Tech Stack:** Rust 2021 workspace, `serde`, `serde_json`, `thiserror`,
`reqwest 0.13.4` blocking HTTPS with Rustls/ring, `url`, `time 0.3.54`,
existing Core evidence/probe contracts, shell release gates.

---

## Execution order

1. [Foundation: Core contracts and bounded transport](2026-07-29-official-macro-foundation.md)
2. [China official data: NBS, PBC, and CFETS](2026-07-29-china-official-macro-providers.md)
3. [Global macro: FRED, IMF, and World Bank](2026-07-29-global-macro-providers.md)
4. [SEC EDGAR filing metadata](2026-07-29-sec-edgar-provider.md)
5. [Public financial-news metadata](2026-07-29-public-financial-news-providers.md)
6. [Router, documentation, admission, and release](2026-07-29-official-data-integration-release.md)

Each plan ends at a working checkpoint and names the commit it creates. Do not
start a later plan while an earlier package checkpoint is red. NBS and DR007
are implemented as explicit diagnostic/unsupported paths unless the exact
official contract described in their plan passes admission during execution.
That outcome is a truthful completion, not a fixture-backed production claim.

## Completion evidence

The implementation is complete only when:

- all six plan checkpoints pass;
- every advertised capability matches a recorded live admission result;
- authenticated FRED operation never logs or serializes the API key;
- CFETS is bounded to Shibor, LPR, and official central parity unless a
  separately proven DR007 contract is added through a new design;
- public news retains no summary, body, description, image, cookie, or inferred
  instrument;
- the committed tree passes the full release command in the final plan.
