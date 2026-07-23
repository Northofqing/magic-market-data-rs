# a-stock-data capability parity task plan

## Goal

Reimplement the useful read-only capabilities demonstrated by
`simonlin1212/a-stock-data` inside the existing Rust Core/Provider architecture,
with strict contracts, deterministic tests, real probes, deployment documents,
and explicit unsupported boundaries.

## Current phase

Phase 3

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

**Status:** in_progress

- Add normalized contracts for the approved non-market-data domains.
- Add checked construction, serde validation and common source evidence.
- Extend router adapters without adding concrete Provider dependencies.

### Phase 4: Provider implementation

**Status:** pending

- Implement provider modules in dependency order with deterministic fixtures.
- Add bounded live/load probes and explicit unsupported/authentication errors.
- Reuse current clients only where source semantics genuinely match.

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
