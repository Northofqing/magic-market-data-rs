# Task Plan: Integrate Local Main with Remote Main

## Goal

Safely integrate local `main` (including Yonhap and WallstreetCN Providers)
with `origin/main`, pass the registered release gates, and push the resulting
non-force merge to the remote `main` branch without touching the user's
uncommitted files in the primary worktree.

## Current Phase

Phase 3

## Phases

### Phase 1: Establish Isolated Baseline

- [x] Fetch current remote state and quantify divergence.
- [x] Preserve the primary worktree's uncommitted files.
- [x] Create an ignored isolated worktree and integration branch.
- [x] Record baseline build and test evidence.
- **Status:** complete

### Phase 2: Merge and Resolve

- [x] Merge `origin/main` without rebasing or rewriting local history.
- [x] Resolve conflicts by preserving both registered capabilities and
  explicit failure/provenance contracts.
- [x] Review the complete merge diff and dependency graph.
- **Status:** complete

### Phase 3: Release Gates

- [ ] Run formatting, workspace tests, strict Clippy, compliance, and
  documentation checks.
- [ ] Run the complete release preflight on the integrated tree.
- **Status:** in_progress

### Phase 4: Publish

- [ ] Re-fetch and confirm `origin/main` has not moved.
- [ ] Fast-forward local `main` to the verified integration commit.
- [ ] Push local `main` to `origin/main` without force.
- [ ] Verify the remote branch resolves to the pushed commit.
- **Status:** pending

### Phase 5: Cleanup

- [ ] Remove the integration worktree and temporary branch.
- [ ] Verify the primary worktree's pre-existing uncommitted files remain.
- **Status:** pending

## Decisions

| Decision | Rationale |
| --- | --- |
| Use an isolated worktree | The primary `main` worktree contains user-owned uncommitted planning and documentation files. |
| Merge rather than rebase | Local and remote histories both contain published-value work; a merge preserves both histories without rewriting commits. |
| Never force-push | The user requested remote integration, not remote history replacement. |
| Re-fetch before push | Prevents overwriting a remote branch that moved during conflict resolution or testing. |

## Errors Encountered

| Error | Attempt | Resolution |
| --- | --- | --- |
| `git pull --ff-only` reported divergent branches | Local merge cleanup | Preserved the explicit failure, fetched the remote state, and moved remote integration into this isolated merge workflow. |
| Merge could not create the linked worktree's `ORIG_HEAD.lock` | First isolated merge attempt | The managed sandbox exposes common Git metadata read-only; retry the same exact merge with repository-scoped elevated permission. |
| Automatic merge reported 26 conflicted files | Provenance-preserving merge | Resolve current shared implementations in favor of the newer remote line, then explicitly preserve and verify the local Yonhap/WallstreetCN identities, routing, documentation, compliance, and packaging registrations. |
| Conflict checkout could not create `index.lock` | First mechanical resolution attempt | The worktree files are writable but its shared Git index metadata is sandboxed; retry the exact selected-file resolution with repository-scoped permission. |
| Planning patch contained an empty update hunk | First error-log update | Reissued a focused patch containing only valid hunks. |
| Planning patch again contained an empty update hunk | Coverage failure log update | Removed the unintended empty findings-file hunk and applied only the two valid planning-file updates. |
| Both RSS crates failed `E0560` on `announcement_discovery` | First integrated all-target check | Remote Core renamed the explicit capability field to `market_announcements`; update both Provider constructors to the new field while preserving `false`. |
| Release preflight stopped at strict Clippy with 27 TDX findings | First integrated preflight | Fixed the reported equivalent-form issues without weakening lint policy: introduced named callback/test tuple types, derived the existing default, removed redundant same-type conversions, used inclusive range checks, and removed no-op error conversions. Strict TDX Clippy and all 344 TDX all-target test cases now pass. |
| Strict coverage evidence was rejected before threshold evaluation | First integrated coverage check | `protocol/types.rs` was newly included in the remote critical glob but retained two inline test bodies. Moved both tests unchanged to the path-based external module `tests/internal/protocol_types.rs`; the focused tests and strict TDX Clippy pass. |
