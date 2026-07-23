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
- Keep commands compatible with the pinned Rust stable toolchain.

## Phases

### Phase 1: Context and README design

**Status:** complete

- Audit the current README, provider contracts, router policy, deployment
  runbook and live evidence.
- Choose an operator-first entry-manual structure.
- Write and self-review the README design.

### Phase 2: Implementation plan

**Status:** complete

- Write an exact documentation implementation plan.
- Self-review the plan for missing sections, placeholders and contradictions.

### Phase 3: README expansion

**Status:** complete

- Replace the sparse root README with the approved Chinese-first entry manual.
- Keep technical identifiers, commands and API names exact.

### Phase 4: Verification and delivery

**Status:** complete

- Run link, compliance and diff checks.
- Review all capability claims against the source documents.
- Commit, push and verify `origin/main`.
- Regenerate and verify the five-probe release package for the final commit.

Completion evidence:

- The README grew from 34 to 477 lines and contains all twelve required
  top-level sections.
- The isolated Rust stable release preflight passed.
- The package for implementation commit `e204167` contained five probes and
  the expanded README; every SHA-256 entry passed and no vendor secret/runtime
  file was present.
- The implementation commit was pushed to `origin/main`.

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
| `check-complete.sh` reported `0/4` because plain status lines are not recognized markers | 1 | Converted all four phase markers to the supported bold form and removed a matching literal from this error row; the completion check then reports 4/4. |
