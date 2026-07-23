# Comprehensive README task plan

## Goal

Turn the root `README.md` into an accurate, self-contained entry point for
developers and operators without duplicating the detailed provider manuals.

## Constraints

- Preserve the user's untracked
  `docs/integrations/stock-analysis-market-data-requirements.md`.
- Describe only capabilities proved by code, deterministic tests or recorded
  live evidence.
- Keep EMQuant device activation separate from API product entitlement.
- Keep provider terms, credentials, activation tokens and vendor binaries out
  of the repository and release package.
- Keep commands compatible with the pinned Rust 1.83.0 toolchain.

## Phases

### Phase 1: Context and README design

Status: complete

- Audit the current README, provider contracts, router policy, deployment
  runbook and live evidence.
- Choose an operator-first entry-manual structure.
- Write and self-review the README design.

### Phase 2: Implementation plan

Status: complete

- Write an exact documentation implementation plan.
- Self-review the plan for missing sections, placeholders and contradictions.

### Phase 3: README expansion

Status: in_progress

- Replace the sparse root README with the approved Chinese-first entry manual.
- Keep technical identifiers, commands and API names exact.

### Phase 4: Verification and delivery

Status: pending

- Run link, compliance and diff checks.
- Review all capability claims against the source documents.
- Commit, push and verify `origin/main`.
- Regenerate and verify the five-probe release package for the final commit.

## Decisions

- Use a Chinese-first README because the operator workflow and current project
  documentation are primarily Chinese; retain Rust/API identifiers in English.
- Put the fast path first, then capability truth, then operational detail.
- Use summary matrices and links instead of copying every provider field table.
- Clearly label live-verified, implemented-but-unverified and unsupported
  boundaries.

## Errors encountered

| Error | Attempt | Resolution |
| --- | --- | --- |
| None | - | - |
