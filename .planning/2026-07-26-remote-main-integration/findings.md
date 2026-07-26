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

## Merge Conflict Strategy

- The merge reported 26 conflicts. Most are shared files in TDX, Eastmoney,
  Exchange, coverage, and preflight that received further fixes on the remote
  unified-release line.
- For shared implementation conflicts, the remote side is the authoritative
  newer baseline. Selecting it avoids reverting the remote's release-gate and
  correctness fixes.
- For aggregate documentation and registration files, start from the remote
  version and explicitly restore the local Yonhap and WallstreetCN additions.
- Unique Provider crates, integration guides, Core identities, Router tests,
  Cargo workspace registration, lockfile entries, and package-script changes
  merged independently and still require post-resolution verification.
- Remote `main` already uses BR-021 through BR-029. The local RSS rules were
  therefore renumbered to BR-030 (Yonhap) and BR-031 (WallstreetCN), and the
  compliance sentinel now requires the complete BR-001 through BR-031 range.
- The package script contains exactly 30 `build_probe` registrations,
  including live/load probes for both RSS Providers.
- After selected-file and aggregate-file resolution, no conflict marker
  remains in README, code, docs, or tools.
- Workspace membership, lockfile packages, Core Provider identities,
  provider-neutral Router tests, integration guides, compliance entries, and
  release-package registrations for both RSS Providers remain present.
- Remote Core renamed the content-capability field from
  `announcement_discovery` to `market_announcements`. Both RSS Providers
  truthfully remain `false`; only the field name must change.
- Dependency inspection confirms Router still depends only on Core and
  `thiserror`; each RSS Provider depends only on Core plus locked registry
  crates. No downstream `stock_analysis` path dependency was introduced.
- The integrated Router intelligence suite contains 24 tests and retains both
  acceptance and identity-mismatch cases for Yonhap and WallstreetCN.
- Rust 1.97 enables strict Clippy findings that the merged TDX implementation
  did not satisfy. All 27 findings were mechanical/equivalent-form issues;
  none required changing protocol behavior, source semantics, or error
  provenance, and no lint allow-list was introduced.
- The remote critical coverage glob now includes every TDX protocol source.
  `protocol/types.rs` was the only matching file whose two tests were still
  inline. Moving those bodies to `tests/internal/protocol_types.rs` preserves
  private-module access and test behavior while keeping test lines out of the
  production coverage denominator.
