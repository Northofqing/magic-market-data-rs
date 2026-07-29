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

## Implementation

- **Status:** complete
- User selected the subagent-driven execution path.
- Started the foundation checkpoint with two non-overlapping workstreams:
  Core provider-neutral contracts and the shared bounded HTTPS transport.
- Both first-pass implementations passed their focused all-target tests,
  strict Clippy, Rustdoc, and diff checks.
- Independent review found two Important Core invariant gaps and four
  Important transport/security gaps. Foundation acceptance is paused while
  test-first repairs replace the logging-unsafe HTTP execution path and close
  direct-construction/header/timeout/URL-normalization bypasses.
- Core review repairs passed a second independent review with no remaining
  Critical or Important findings and were committed as `6f7079b`.
- Transport review repairs passed a third independent review with no remaining
  Critical or Important findings. The safe Reqwest/Rustls implementation and
  plan correction were committed as `9a18f69`.
- The complete Foundation checkpoint passed formatting, Core and transport
  all-target tests, strict Clippy, Rustdoc, and `git diff --check`.
- Registered all ten source crates in the workspace and committed the common
  provider scaffold as `7ad6b04`.
- Completed deterministic NBS, PBC, CFETS, FRED, IMF, World Bank, and SEC
  implementations. Production capability remains false and fails before I/O
  wherever current live evidence or required source facts are absent.
- Completed metadata-only Xinhua Finance, Yicai, and Securities Times clients,
  strict full-page parsers, injected transport tests, live/load probes, and
  rights-boundary READMEs.
- Provider-neutral economic, reference-rate, official-fixing, filing, and
  new-news-identity Router integration passed focused tests, strict Clippy,
  and final review with zero Critical/Important findings; committed as
  `8dfc38e`.
- Real admission on 2026-07-29 passed for the exact PBC 2024 money-supply
  catalog, CFETS Shibor/LPR/official FX, and Xinhua/Yicai/STCN first-page
  metadata. Each completed two consecutive live probes and a three-call
  serial load probe before its capability flag was enabled.
- NBS remains diagnostic-only despite a successful 140,978-byte landing-page
  probe because no supported machine-series contract was proved. FRED and SEC
  were not run without their required runtime identity values. IMF returned
  HTTP 403, World Bank exposed an empty structured unit, and CFETS DR007 has no
  equivalent audited public history contract; those flags remain false.
- Independent final reviews of Core/Router, transport/China sources, global
  macro/SEC, and public news each reported zero remaining Critical/Important
  findings. STCN terminal-empty handling was tightened to return a protocol
  error rather than an unproved ordinary empty batch before the news review
  closed.

## Release Validation

- **Status:** in progress
- Moved critical-source inline test bodies into path-based external test
  modules and added boundary coverage for source identity, pagination,
  malformed envelopes, timestamps, and explicit unavailable states.
- Removed three duplicate Router checks whose invalid states cannot be
  constructed through the checked Core contracts.
- A clean `cargo llvm-cov --workspace --all-features --no-report` run passed
  all production tests. The repository checker then passed at
  `45230/51259 = 88.24%` overall and `26322/27707 = 95.00%` for the critical
  data path.
- Ordinary stable-toolchain doctests passed separately. LLVM doctest
  persistence was not treated as release evidence because cargo-llvm-cov
  requires nightly-only `-Z persist-doctests` for that mode.
- The full release preflight passed on the staged final tree, including
  formatting, all-target/all-feature checks and tests, strict Clippy, Rustdoc,
  stable doctests, documentation links, compliance, and the supplied clean
  coverage evidence.
- Package verification, final diff review, and main-branch integration remain.
