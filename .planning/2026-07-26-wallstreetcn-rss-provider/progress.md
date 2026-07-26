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

- **Status:** complete
- Started Core Provider identity and provider-neutral Router evidence tests.
- Core red test failed with two `E0599` errors because
  `ProviderId::WallstreetCn` did not exist.
- Router red test failed with four `E0599` errors for the same missing
  identity. No unrelated compile or contract failure appeared.
- The first formatting check found one rustfmt-only wrapping difference in the
  new Router test; `cargo fmt --all` applied the canonical layout.
- Core identity tests passed 3/3 and Router intelligence tests passed 16/16,
  including WallstreetCN acceptance and evidence-mismatch rejection.
- Registered the standalone `magic-wallstreetcn-rs` crate without adding a new
  registry dependency resolution.
- Added the one exact request URL, closed RSS-compatible MIME set, 2 MiB body
  bound, 1–60 second timeout bound, 1–50 caller limit, typed status/network
  failures, injected-response revalidation, and clone-shared pacing.
- Request, response-bound, status, and shared-gate tests all passed.

## Phase 2

- **Status:** in progress
- Parser red failed only because `rss::parse_response` was absent.
- Added a complete-feed `quick-xml` state machine with an RSS 2.0 root,
  exact channel identity, required item fields, ignored description/extension
  subtrees, strict entity handling, a 100-item source bound, canonical article
  URLs and decimal IDs, RFC 2822 source times, newest-first ordering, and
  duplicate rejection before caller truncation.
- Metadata mapping emits no summary, content, or inferred instruments and uses
  `ProviderId::WallstreetCn` with `wallstreetcn-rss-v1` provenance.
- Added `NewsProvider` boundaries: instrument news is typed unsupported,
  diagnostic global-news fetches are available, and public global news remains
  truthful to `GLOBAL_NEWS_ADMITTED`.
- Parser tests passed 9/9, capability tests passed 5/5, all crate tests passed
  21/21, and strict crate Clippy, formatting, and diff checks passed.
- Added bounded live and serial-load examples with pure configuration tests.
- At 2026-07-26 08:40:27 +0800, the release live probe passed with 20
  complete, newest-first, metadata-only records from `wallstreetcn-rss-v1`.
  The newest source time was `2026-07-25T19:35:51+08:00`.
- At 2026-07-26 08:41:15 +0800, the release load probe passed two serial
  production-client requests through one client. Each returned 10 records;
  total elapsed time was 7.529 seconds and all source/provider/batch/content
  invariants passed.
- Because both admission commands passed, set `GLOBAL_NEWS_ADMITTED=true`.
- The first optional `9500亿美元` title-match attempt returned a typed DNS
  `Transport` failure. The unchanged bounded release probe was rerun outside
  the sandbox and passed at fetched time `2026-07-26T03:32:52.311953Z`,
  matching article ID `3777926`.
- After admission, all crate targets passed 24/24 tests, strict crate Clippy
  passed, and Router intelligence tests passed 16/16.
