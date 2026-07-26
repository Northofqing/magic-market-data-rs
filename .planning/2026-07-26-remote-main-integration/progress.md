# Progress: Remote Main Integration

## 2026-07-26

- User selected local merge first; local `main` was fast-forwarded to
  `32ae56f`, the full workspace/all-target test suite passed, and the feature
  worktree and branch were cleaned up.
- User then requested remote integration.
- `git pull --ff-only` fetched `origin/main` to `13b0172` but stopped without
  modifying local history because local and remote `main` diverged.
- Root-cause inspection found 37 local-only and 78 remote-only commits.
- Chose a merge, not a rebase or force-push, to preserve both histories.
- Created branch `integrate/remote-main-20260726` in the ignored isolated
  worktree `.worktrees/remote-main-integration`.
- Primary-worktree uncommitted files remain outside the integration worktree.
- The exact local baseline commit `32ae56f` had already passed
  `cargo test --workspace --all-targets --locked --offline` in the primary
  worktree immediately before isolation; the integration worktree starts from
  that same commit.
- The first isolated merge attempt stopped before changing the index because
  the sandbox could not create the linked worktree's `ORIG_HEAD.lock`.
  Repository-scoped elevated permission is required for this Git metadata
  write.
- The elevated merge reached the content phase and reported 26 conflicts.
  No conflict was auto-resolved by discarding history; the merge remains open
  for file-by-file resolution.
- The first selected-file conflict checkout was blocked before changing files
  because the linked worktree could not create `index.lock`; the retry requires
  repository-scoped permission.
- The first attempt to log that failure used an invalid empty patch hunk; a
  focused planning-only patch succeeded without touching merge content.
- Selected the remote unified-release versions for 22 shared implementation,
  test, exchange-guide, coverage, and preflight conflicts.
- Manually reconciled README, deployment, business rules, and compliance so
  remote release semantics and local Yonhap/WallstreetCN registrations both
  remain.
- Renumbered the two local RSS rules to BR-030/BR-031 because remote already
  owns BR-021 through BR-029.
- Conflict-marker scan is clean; the remaining unmerged index status now only
  requires staging the reviewed resolutions.
- First integrated checks: formatting, documentation links, and compliance
  passed. The all-target check failed only in the two RSS crates because
  remote Core renamed `announcement_discovery` to `market_announcements`.
- Updated both constructors to `market_announcements: false`. The complete
  workspace/all-target check then passed.
- Focused verification passed:
  - WallstreetCN: 26/26 all-target tests;
  - Yonhap: 30/30 all-target tests;
  - Core Provider identity: 3/3;
  - Router intelligence routing: 24/24.
- Dependency trees preserve Core-only Provider boundaries and a
  Provider-neutral Router.
