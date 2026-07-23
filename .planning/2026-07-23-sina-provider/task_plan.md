# Sina provider task plan

## Goal

Add a production-honest Sina public-market-data provider to the Rust workspace,
prove every advertised capability with deterministic and live evidence, document
its deployment boundary, package its probes, and deliver the result to Git.

## Constraints

- Preserve the user's untracked
  `docs/integrations/stock-analysis-market-data-requirements.md`.
- Use only public Sina market-data endpoints; do not capture credentials,
  cookies, account data or private client traffic.
- Advertise only data families whose response contract and source time are
  strictly validated.
- Unsupported or unverified families must return explicit errors.
- Keep bounded timeouts, response sizes, request counts and concurrency.
- Remain compatible with rolling stable Rust/Cargo and `unsafe_code = "forbid"`.

## Phases

### Phase 1: Restore context and verify Sina contracts

**Status:** complete

- Audit the existing Tencent provider, Core contracts, release tooling and docs.
- Probe official Sina quote/K-line/minute/trade endpoints with real securities.
- Record response semantics, limits, timestamps and unsupported boundaries.

### Phase 2: Design and implementation plan

**Status:** complete

- Compare integration approaches and select the smallest honest capability set.
- Write and self-review the design specification.
- Write and self-review an exact TDD implementation plan.

### Phase 3: Provider implementation

**Status:** complete

- Add `magic-sina-rs` with strict parsing, bounded HTTP and capability traits.
- Add deterministic unit/contract tests and live/load probes.
- Integrate workspace, docs, release package and compliance policy.

### Phase 4: Verification and delivery

**Status:** complete

- Run deterministic, live, load, strict lint, docs and release gates.
- Review capability claims and security/deployment boundaries.
- Commit, push, package the final commit and verify its manifest.

## Decisions

- Treat Sina as a public-web supplemental provider, not an SLA-backed primary
  source.
- Reuse Core contracts and the mature Tencent provider shape where the Sina
  response semantics genuinely match.
- Use the official public Quote plus K-line endpoints. Implement current
  minute data by accumulating the latest date's bounded 1-minute K-line window.
- Normalize all Sina source share quantities to lots at the provider boundary.
- Leave Trades, MoneyFlow and Auction disabled rather than parse presentation
  HTML or derive unsupported business fields.

## Errors encountered

| Error | Attempt | Resolution |
| --- | --- | --- |
| Cargo could not find package `magic-sina-rs` | 1 | Expected TDD red state before adding the workspace member and crate scaffold. |
| Quantity-without-price test was rejected earlier as a top-of-book contradiction | 1 | The fixture changed level-one price but not the redundant best-bid summary. Changed both fields consistently so the intended partial-book path is isolated. |
| First live probe could not resolve `hq.sinajs.cn` inside the restricted sandbox | 1 | Re-ran the same probe with the approved Sina network permission; all supported families passed. |
| Activated EMQuant `CSD` returned `invalid bar time range` | 1 | Captured the raw SDK response, reproduced `YYYY/M/D` in the fake bridge and added strict ISO zero-padding. The regression and real five-bar query now pass. |
