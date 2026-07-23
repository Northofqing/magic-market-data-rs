# Task 8 live/load probe gates

## Goal

Make public-provider probe exit status a truthful, machine-checkable admission
result without renaming production Provider APIs.

## Phases

1. **Gate A + shared verifier + THS empty semantics** — in progress
2. **Eastmoney integrity** — pending
3. **iWencai status truthfulness** — pending
4. **Baidu admission and CLS/CNInfo gates** — pending
5. **Measured load coverage and docs** — pending
6. **Scoped verification and handoff** — pending

## Decisions

- `admitted` and source-evidenced `verified_empty` are the only capability
  success states.
- `diagnostic_complete_unadmitted`, `skipped_missing_secret`, and `failed`
  never satisfy capability admission.
- Ordinary empty `DataBatch`, incomplete quality, issues, provenance mismatch,
  future/stale source time, and duplicate identity fail explicitly.
- Tests moved from touched production files use path-based test modules so
  private seams stay private and test bodies do not inflate production-file
  coverage.

## Validation

- Focused crate tests for each red/green slice
- Scoped examples build without network execution
- `cargo fmt --check`
- Scoped strict Clippy and rustdoc
- Compliance and documentation checks

## Rollback

Each phase is a separate commit and can be reverted with `git revert <sha>`.
If the shared verifier proves incompatible, revert it while retaining
provider-local explicit failure checks.

## Errors

| Error | Resolution |
| --- | --- |
| Initial worktree creation could not write the main repository ref under sandboxing | Used the approved scoped `git worktree add`; exact base remains `45c75be` |
| First oversized patch was interrupted | Verified the tree was unchanged and split work into small patches |
| `tools/compliance/lib/check_business_rules.sh` does not exist in this repository | Use the repository-level `tools/compliance/check.sh` at the final scoped gate; retain `git diff --check` for the docs-only commit |
