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

- **Status:** complete
- User approved the first-batch source scope.
- User approved source-aligned Provider crates, new provider-neutral Core
  contracts, strict provenance, bounded transports, and live admission gates.
- Audited the existing Core calendar, global snapshot, content, evidence,
  batch, and Provider identity contracts.
- Completed and self-reviewed the written specification required before a
  file-by-file implementation plan.
- User approved the committed written specification.

## Detailed Implementation Planning

- **Status:** complete
- Started the required test-first file mapping.
- Split the work into foundation, China official data, global macro, SEC,
  public financial news, and final integration/release plans.
- The first direct NBS page audit returned HTTP 403 to a minimal curl client;
  no bypass was attempted and the planned capability remains diagnostic-only
  until a compliant production request is proved.
- Completed a 3,600-line executable plan set with exact paths, red/green
  commands, code shapes, source-specific admission outcomes, review steps,
  coverage/package registration, and clean-tree release gates.
- Self-review confirmed all ten Provider identities are covered, plan links
  resolve, `git diff --check` passes, and no unfinished-marker strings remain.
- Corrected the CFETS central-parity positional mapping to use the selected
  `currency`/`searchlist` order rather than the full catalog heading list.
- Recorded truthful blockers for NBS, PBC social financing, CFETS DR007, and
  World Bank structured units without weakening the approved contracts.
- Committed the reviewed plan set as `1e861ed`.
