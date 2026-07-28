# Progress: Official Macro, Global Data, SEC, and Financial News

## Baseline

- **Status:** complete
- Created isolated worktree
  `/Users/zhangzhen/Desktop/Quant/magic-market-data-rs/.worktrees/official-macro-global-news`.
- Created branch `feat/official-macro-global-news` from `origin/main` at
  `660902f`.
- Confirmed a clean worktree before design work.
- `cargo build --workspace --locked --offline` passed.
- `cargo test --workspace --all-targets --locked --offline` passed.

## Design

- **Status:** in progress
- User approved the first-batch source scope.
- User approved source-aligned Provider crates, new provider-neutral Core
  contracts, strict provenance, bounded transports, and live admission gates.
- Audited the existing Core calendar, global snapshot, content, evidence,
  batch, and Provider identity contracts.
- Completed and self-reviewed the written specification required before a
  file-by-file implementation plan.
