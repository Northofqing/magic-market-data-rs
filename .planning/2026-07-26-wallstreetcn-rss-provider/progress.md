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

- **Status:** complete
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

## Phase 3

- **Status:** complete
- Registered WallstreetCN in the root and Provider READMEs, deployment and
  integration guidance, `BR-022`, upstream-source documentation, compliance
  checks, release packaging, and live-probe packaging.
- Packaging expects exactly 30 Provider probes after the WallstreetCN live and
  load probes were added.
- Documentation links, compliance checks, and the package manifest all passed.

## Phase 4

- **Status:** in progress
- Strict coverage passed:
  - overall: 23,514 / 29,277 lines, 80.32% (required 80%)
  - critical: 1,881 / 1,960 lines, 95.97% (required 95%)
- The following release checks passed from the isolated feature worktree:
  - `cargo fmt --all -- --check`
  - `cargo check --workspace --all-targets --locked --offline`
  - `cargo test --workspace --all-targets --locked --offline`
  - `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`
  - `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked --offline`
  - `cargo test --workspace --doc --locked --offline`
  - `bash tools/docs/check_links.sh`
  - `bash tools/compliance/check.sh`
  - `bash tools/release/preflight.sh`
- The complete preflight independently rebuilt and tested all workspace targets
  in its isolated target directory and ended with
  `release preflight: passed`.
- `git diff --check` passed. Dependency inspection confirmed that the
  WallstreetCN crate depends on Core plus registry dependencies only, Router
  still depends only on Core plus `thiserror`, and no downstream
  `stock_analysis` path dependency was introduced.
- Cargo emitted the existing workspace-wide warning for same-named Provider
  examples (`live_probe` and `load_probe`). It is non-fatal, and release
  packaging assigns Provider-specific output names.
- Independent review found no Critical issues, two Important XML strictness
  gaps, and one related Minor allocation/documentation issue:
  - ignored text, attributes, comments, and processing instructions did not all
    receive XML 1.0 legal-character validation;
  - XML declarations were not required to have a unique, ordered
    `version="1.0"` contract;
  - ignored Text and CDATA were decoded before being discarded.
- Added two adversarial parser tests first. Both failed on the prior
  implementation, reproducing the review findings.
- Added document-wide XML 1.0 character validation, decoded-attribute
  validation, quick-xml comment checking, unique and ordered declaration
  validation, and a no-decode path for ignored Text and CDATA. The focused
  rejection tests then passed.
- All WallstreetCN targets passed 26/26 after the fix, including 18 library
  tests, 5 capability tests, and 3 example-configuration tests. Formatting,
  strict crate Clippy, and `git diff --check` passed.
- The post-fix release live probe passed with 20 complete metadata-only rows at
  fetched time `2026-07-26T04:28:48.638367Z`; article `3777926` remained
  present. The post-fix load probe used one client for two serial requests,
  returned 10 rows each, and passed in 9.295 seconds.
- Strict coverage was regenerated after the production change and passed at
  23,514 / 29,277 = 80.32% overall and 1,881 / 1,960 = 95.97% critical.
- The complete isolated release preflight was rerun after the fix and ended
  with `release preflight: passed`.
- Follow-up independent review of the remediation remains before handoff.
