# Findings: Remote Main Integration

## Baseline

- Local `main`: `32ae56f1973db9785678cd5df3aabe0e2b1b1a12`.
- Remote `origin/main`: `13b0172` after the 2026-07-26 fetch.
- Divergence at the start of integration: local ahead 37, behind 78.
- The local WallstreetCN/Yonhap line descends from the pre-integration local
  `main`; the remote line contains a separate unified-data release history.
- A fast-forward pull is impossible. Rebasing would rewrite the local commit
  line, so a merge commit is the safe provenance-preserving operation.

## Workspace Safety

- The primary worktree contains a modified `.planning/.active_plan`, several
  untracked `.planning/...` directories, and an untracked stock-analysis
  integration document.
- Those files are user-owned and must not be staged, reset, overwritten, or
  removed.
- `.worktrees/` is ignored by `.gitignore`; the integration worktree is
  isolated at `.worktrees/remote-main-integration`.

