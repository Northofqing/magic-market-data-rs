# Sina instrument-news provider plan

## Goal

Add a production-honest `magic-sina-rs` implementation of
`NewsProvider::instrument_news` for Sina's current official A-share company
news page, with strict A-share request identity, bounded paging/filtering,
complete source evidence, deterministic duplicate handling, and a real
bounded probe.

## Constraints

- Do not edit downstream `stock_analysis` in this upstream slice.
- Do not touch TDX, Eastmoney, dragon-tiger, or unrelated dirty files.
- Reject the retired `feed.mix` pageid=155 route. Admit only the audited
  `https://vip.stock.finance.sina.com.cn/corp/view/vCB_AllNewsStock.php?symbol={symbol}&Page={page}`
  company-news page and its bounded HTTPS pagination shape.
- Never infer an instrument from title or body text.
- Keep `global_news` explicitly unsupported without independent live proof.
- Do not commit or push.

## Phases

### Phase 1: Context and contract design

**Status:** complete

- Audit Core news/request contracts, existing Sina transport, old downstream
  parser, comparable providers, and the live endpoint.
- Compare implementation shapes and select the smallest strict boundary.
- Write and self-review the focused design plus BR before code.

### Phase 2: TDD implementation

**Status:** complete

- Implement vertical RED/GREEN slices for request identity, transport/MIME,
  parsing/evidence, filtering/paging, duplicates, and unsupported global news.
- Keep all failures explicit and preserve exact provider/source evidence.

### Phase 3: Live probe and documentation

**Status:** complete

- Add a bounded real probe and capability/README truth.
- Record real endpoint MIME/shape and normalized evidence.

### Phase 4: Verification and handoff

**Status:** complete

- Run crate format, tests, strict Clippy, compliance, docs, and real probe.
- Review changed paths and report any Gate D limitation without claiming merge
  readiness.

## Decisions

- The parent brief is the approved capability contract; no further
  clarification gate is required.
- Prefer a dedicated `news.rs` deep module over embedding parsing/paging in
  `lib.rs`.
- Keep Core public request/provider trait names unchanged.

## Errors encountered

| Error | Attempt | Resolution |
| --- | --- | --- |
| Upstream worktree lacks local `CLAUDE.md`/engineering companion files | 1 | Read the authoritative copies from the parent `stock_analysis` repository plus the upstream `AGENTS.md`. |
| Planning-file update missed an exact heading | 1 | Re-read the files and applied a context-correct update; no production file changed. |
| Legacy `feed.mix` instrument page is no longer registered | 1 | Parent selected audited official company-news HTML as the replacement source; exact request symbol and page marker must match. |
| Filtered-empty regression returned `Protocol`, and an equivalent duplicate fetched one second later conflicted | 1 | RED is expected: construct a complete empty batch from source-backed page provenance and compare duplicate source facts without local observation time. |
| Combined documentation patch missed an exact Sina integration paragraph | 1 | Split the patch by document and use the current section text as context; no documentation change was partially applied. |
| `cargo fmt -p magic-sina-rs -- --check` reported formatting diffs | 1 | Ran package-scoped `cargo fmt -p magic-sina-rs`; unrelated workspace crates were not formatted. |
| First live probe failed DNS inside the restricted sandbox | 1 | Re-ran the same bounded read-only probe with approved network access. |
| Shanghai live probe passed, Shenzhen failed because the official page supplied an HTTP Sina article URL | 1 | Verified the exact same host/path over HTTPS returns HTTP/2 200 with GBK HTML and no redirect; register and test scheme-only HTTPS canonicalization before retrying. |
| First URL-normalization code patch missed rustfmt-adjusted context | 1 | Re-read the exact function and applied a smaller context-specific patch. |
| Locked focused test refused the newly declared direct `url` dependency | 1 | Regenerated only Cargo.lock dependency metadata offline, then reran the focused test. |
| Final Shenzhen fixture addition needed rustfmt normalization | 1 | Ran package-scoped rustfmt, then reran fmt check, all-target tests and strict Clippy successfully. |
