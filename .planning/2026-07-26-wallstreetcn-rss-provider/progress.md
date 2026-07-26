# Progress: WallstreetCN RSS News Provider

## Baseline

- Approved design:
  `docs/superpowers/specs/2026-07-26-wallstreetcn-rss-provider-design.md`
  at commit `c2f6348`.
- Detailed TDD implementation plan:
  `docs/superpowers/plans/2026-07-26-wallstreetcn-rss-provider.md`
  at commit `0f63041`.
- Execution mode: inline in the existing isolated feature worktree.
- Yonhap work preceding this feature passed strict coverage and the complete
  release preflight before WallstreetCN implementation began.

## Phase 1

- **Status:** in progress
- Started Core Provider identity and provider-neutral Router evidence tests.
- Core red test failed with two `E0599` errors because
  `ProviderId::WallstreetCn` did not exist.
- Router red test failed with four `E0599` errors for the same missing
  identity. No unrelated compile or contract failure appeared.
- The first formatting check found one rustfmt-only wrapping difference in the
  new Router test; `cargo fmt --all` applied the canonical layout.
- Core identity tests passed 3/3 and Router intelligence tests passed 16/16,
  including WallstreetCN acceptance and evidence-mismatch rejection.
