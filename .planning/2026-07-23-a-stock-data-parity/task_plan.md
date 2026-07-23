# a-stock-data capability parity task plan

## Goal

Reimplement the useful read-only capabilities demonstrated by
`simonlin1212/a-stock-data` inside the existing Rust Core/Provider architecture,
with strict contracts, deterministic tests, real probes, deployment documents,
and explicit unsupported boundaries.

## Current phase

Phase 4

## Constraints

- Preserve `docs/integrations/stock-analysis-market-data-requirements.md`
  untouched and uncommitted.
- Treat the reference repository as protocol and capability research, not as
  code to copy blindly.
- Preserve Rust 1.83 compatibility, `unsafe_code = "forbid"`, bounded network
  access, record-level provenance, typed errors and no simulated success.
- Do not ingest credentials, account state, trading data or private client
  traffic.
- Separate independently deployable business domains into their own Core
  contracts and Provider crates/modules.
- Verify current endpoint behavior before advertising a capability.

## Phases

### Phase 1: Reference and workspace audit

**Status:** complete

- Inventory every reference feature, source, endpoint, data shape and runtime
  dependency.
- Map each feature to existing Core/Provider coverage or a missing contract.
- Identify licensing, authentication, anti-bot and source-time constraints.

### Phase 2: Architecture and staged specifications

**Status:** complete

- Compare viable integration approaches and select one.
- Decompose the ten-layer product into independently testable delivery slices.
- Write and self-review the architecture specification and exact plans.

### Phase 3: Core domain expansion

**Status:** complete

- Add normalized contracts for the approved non-market-data domains.
- Add checked construction, serde validation and common source evidence.
- Extend router adapters without adding concrete Provider dependencies.

### Phase 4: Provider implementation

**Status:** in_progress

- Implement provider modules in dependency order with deterministic fixtures.
- Add bounded live/load probes and explicit unsupported/authentication errors.
- Reuse current clients only where source semantics genuinely match.
- Execute
  `docs/superpowers/plans/2026-07-23-tencent-sina-intelligence.md` as Slice B.
- Slice B Task 1 (Core option-contract widening) is complete.
- Slice B Task 2 (Tencent market statistics) is complete.
- Slice B Task 3 (Sina financial statements) is complete.
- Slice B Task 4 (Sina ETF options) is complete.
- Slice B Task 5 (release gate, commit and push) is complete at
  `ca165beb374080e32403548983b91ea24486bd1f`.
- Execute
  `docs/superpowers/plans/2026-07-23-public-intelligence-providers.md` as
  Slices C and D. Shared Core/Router/workspace files are a main-thread barrier;
  isolated Provider crates may be implemented in parallel after it passes.
- Then implement Eastmoney broad intelligence families, CNInfo/Tonghuashun,
  CLS/Baidu/Iwencai and exchange announcement sources in separate reviewed
  slices.

### Phase 5: Integration and acceptance

**Status:** pending

- Add documentation, deployment, compliance and release packaging.
- Run deterministic, live, load and cross-source verification.
- Complete review, commit, push and verify the final release package.

## Initial decisions

| Decision | Rationale |
| --- | --- |
| Decompose by normalized business domain | The reference spans ten independent systems and is too large for one safe implementation batch. |
| Keep providers below Core contracts | Matches the current architecture and prevents endpoint-specific schemas leaking into consumers. |
| Require real evidence per advertised family | The reference includes public-web sources whose endpoint stability and permissions can change. |

## Errors encountered

| Error | Attempt | Resolution |
| --- | ---: | --- |
| Reference/local combined read used the wrong working directory for `SKILL.md` | 1 | Split subsequent reference and local reads by explicit working directory. |
| Core audit used `magic-market-core/...` instead of workspace member path `crates/magic-market-core/...` | 1 | Correct all subsequent crate reads to use the `crates/` prefix. |
| Core test could not write a fingerprint because the data volume had only 123 MiB free | 1 | Diagnosed `target/debug` at 547 MiB, removed only that reproducible cache, then reran the identical test successfully with 669 MiB free. |
| Tuple Router closures needed explicit request types; first mechanical patch matched the first four generic closures | 2 | Inspected exact matches, restored the four baseline closures, annotated only the four tuple adapters, and passed the full Router suite. |
| New analysis workspace member required a local Cargo.lock package entry | 1 | Ran Cargo once with `--offline` but without `--locked`; it updated only the lock metadata and produced the expected missing-API RED result. |
| Limit-pool duplicate detection required a hashable pool kind | 1 | Added `Hash` to the pure enum and reran the analysis tests successfully. |
| Staged diff check found one blank line at EOF in the new analysis manifest | 1 | Removed the reproducible formatting defect, restaged only the manifest and passed the staged diff check. |
