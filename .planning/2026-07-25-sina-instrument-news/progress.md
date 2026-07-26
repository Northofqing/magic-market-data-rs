# Sina instrument-news progress

## 2026-07-25

- Read upstream and parent repository rules plus the brainstorming,
  planning-with-files, implement, and TDD skills.
- Restored the prior completed Sina provider plan and audited the initial Core,
  Sina, and downstream-provider search surface.
- Published the required pre-flight before file edits.
- Created this isolated plan without changing the shared `.active_plan`.
- Inspected Core content, request, evidence/provenance, validated-value, batch,
  and provider contracts plus the current Sina transport/client boundary.
- Confirmed MIME-aware news transport must be additive and preserve existing
  byte-only test transports.
- One planning-file patch initially missed the exact section heading; no
  production file was changed, the file was re-read, and this corrected update
  records the incident.
- Probed the retired `feed.mix` pageid=155 route and proved it now fails at the
  business-protocol layer.
- Probed pageid=153 and proved its `k` parameter does not provide instrument
  identity.
- Parent selected replacement option A: audit and use Sina's official
  server-rendered company-news page.
- Probed the official `sh600396` company-news page: HTTP 200, no redirect,
  GBK HTML MIME, exact server-side symbol marker, full publication timestamps,
  HTTPS canonical article URLs, and bounded pagination are present.
- Added public-contract integration tests before production code.
- RED confirmed with `cargo test -p magic-sina-rs --test instrument_news`:
  missing `DocumentResponse`, metadata transport method, `NewsProvider`
  implementation, and content capability declaration caused the expected
  compile failures.
- Added the downstream-required verified-empty distinction test. Focused RED
  reproduced two exact defects: valid source rows filtered outside the
  requested date range returned `Protocol`, and equivalent canonical records
  fetched on separate pages conflicted solely because local observation time
  differed.
- Implemented complete filtered-empty batches with page provenance and newest
  fetched source time, plus source-fact-only duplicate equivalence.
- Focused GREEN: `cargo test -p magic-sina-rs --test instrument_news` passed
  all 6 tests in the isolated target directory.
- Corrected the design and BR-016 to distinguish provider-proven filtered
  empty from malformed empty pages.
- Updated the Sina integration contract with the endpoint, evidence,
  capability, bounds, retired-feed disposition and probe command.
- Added a bounded live probe fixed to one Shanghai and one Shenzhen equity,
  limit 3 each, with strict batch and record evidence checks.
- Package fmt check initially found ordinary Rust formatting differences.
  Package-scoped formatting completed without touching unrelated crates.
- Package all-target/all-feature locked/offline tests passed: 32 library
  tests, 1 capability test, 6 instrument-news tests, 4 load-probe tests and
  all example harnesses.
- Extended the public capability contract test so `instrument_news=true`
  cannot diverge from the implemented `NewsProvider` trait.
- Strict all-target/all-feature Clippy passed with `-D warnings`.
- The first real probe needed network escalation after sandbox DNS failure.
  Shanghai returned 3 admitted records. Shenzhen then failed closed on an
  official HTTP article URL; an independent HTTPS probe of the identical
  host/path returned 200 without redirect, so a documented scheme-only
  canonicalization slice is now required.
- Registered the scheme-only normalization in BR-016/design/integration docs.
  Added the regression first; focused RED failed with Core `https_url must use
  https`, proving the live Shenzhen defect is captured.
- Implemented structured URL parsing, Sina-host admission, credential/explicit
  port rejection and scheme-only HTTP-to-HTTPS normalization. Focused GREEN
  passed after an offline Cargo.lock metadata refresh.
- Removed article titles from the live-probe output so the bounded diagnostic
  prints only instrument identity, source/provenance times, batch identity and
  canonical URLs.
- Final package format check passed.
- Final locked/offline all-target/all-feature test run passed: 32 library
  tests, 1 public capability test, 8 instrument-news contract tests, 4
  load-probe tests and every example harness.
- Final locked/offline all-target/all-feature strict Clippy passed with
  `-D warnings`.
- Final bounded release probe passed for both fixed instruments. Shanghai
  `600396` and Shenzhen `000001` each returned 3 non-empty, complete,
  source-evidenced records; the Shenzhen HTTP article edge normalized to the
  independently verified HTTPS identity.
- Added a deterministic Shenzhen regression so `sz000001` URL generation,
  page identity and normalized instrument association are locked independently
  of the live probe.
- Repository documentation-link and compliance scripts both passed without
  output. Scoped `git diff --check` also passed.
