# Task Plan: Integrate Local Main with Remote Main

## Goal

Safely integrate local `main` (including Yonhap and WallstreetCN Providers)
with `origin/main`, pass the registered release gates, and push the resulting
non-force merge to the remote `main` branch without touching the user's
uncommitted files in the primary worktree.

## Current Phase

Phase 2

## Phases

### Phase 1: Establish Isolated Baseline

- [x] Fetch current remote state and quantify divergence.
- [x] Preserve the primary worktree's uncommitted files.
- [x] Create an ignored isolated worktree and integration branch.
- [x] Record baseline build and test evidence.
- **Status:** complete

### Phase 2: Merge and Resolve

- [ ] Merge `origin/main` without rebasing or rewriting local history.
- [ ] Resolve conflicts by preserving both registered capabilities and
  explicit failure/provenance contracts.
- [ ] Review the complete merge diff and dependency graph.
- **Status:** in_progress

### Phase 3: Release Gates

- [ ] Run formatting, workspace tests, strict Clippy, compliance, and
  documentation checks.
- [ ] Run the complete release preflight on the integrated tree.
- **Status:** pending

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
