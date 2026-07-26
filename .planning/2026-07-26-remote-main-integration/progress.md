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
- The first complete release preflight passed all workspace/all-target tests
  and then stopped at strict Clippy with 27 TDX findings. The findings were
  limited to equivalent-form static checks: redundant casts, manual range
  checks, no-op conversions, a derivable default, and complex type spellings.
- Corrected every reported TDX finding without adding lint exemptions.
  `cargo clippy -p magic-tdx-rs --all-targets --all-features --locked
  --offline -- -D warnings -D clippy::all` now passes.
- The complete TDX all-target suite passes after the cleanup: 344 test cases
  (312 library tests plus integration/example targets), with no failures.
- The second complete release preflight passed every registered step:
  formatting, coverage-checker regression tests, workspace all-feature check,
  workspace all-target tests, strict Clippy, Rustdoc, doctests, documentation
  links, and compliance.
- The first real llvm-cov report generated successfully, but the checker
  rejected it before threshold evaluation because the newly critical
  `protocol/types.rs` still contained two inline tests. Moved those test
  bodies unchanged into a path-based external test module. Both focused tests
  and strict TDX Clippy pass after the move.
- Regenerated coverage from clean profiles on the committed external-test
  layout. The repository checker passed both release thresholds:
  - overall production coverage: 33,669 / 38,912 = 86.53% (required 80%);
  - critical data-chain coverage: 15,476 / 16,217 = 95.43% (required 95%).
- Re-ran the complete release preflight on final evidence commit `14315e7`;
  every registered gate passed again.
- Re-fetched `origin/main` and confirmed it remained at `13b0172`, which is an
  ancestor of the verified integration commit.
- Fast-forwarded local `main` without touching the primary worktree's
  pre-existing uncommitted files, then pushed without force.
- Verified `refs/heads/main` on the remote resolved to
  `14315e732c6425c7fad1649d44b913ba8c264ae1`.
