# Public Intelligence Provider Implementation Plan

**Goal:** Complete the reference-comparison public intelligence families with
provider-neutral Core contracts, isolated read-only Provider crates,
deterministic fixtures, truthful live evidence, bounded load probes and
deployment documentation.

**Architecture:** Main owns the shared Core/Router/workspace barrier. After that
barrier passes, provider crates are implemented in parallel and may depend only
on `magic-market-core`. Public-web endpoints are experimental supplemental
sources: HTTPS only, hostname allowlists, no redirects, bounded responses,
bounded page sizes, conservative request pacing, explicit authentication errors
and no reuse of desktop login state, Cookies or account data.

**Constraints:** current default/stable Rust without a repository version pin,
`unsafe_code = "forbid"`, no simulated success, no credential logging,
record-level `SourceEvidence`, no undocumented field inference, no production
claim without a successful real probe.

---

## Task 1: Widen shared Core contracts and routing tests

**Files:**

- Modify: `crates/magic-market-core/src/capital.rs`
- Modify: `crates/magic-market-core/src/content.rs`
- Modify: `crates/magic-market-core/src/limit_pool.rs`
- Modify: `crates/magic-market-core/src/research.rs`
- Modify: `crates/magic-market-core/src/signals.rs`
- Modify: `crates/magic-market-core/tests/*.rs`
- Modify: `crates/magic-market-analysis/tests/analysis.rs`
- Modify: `crates/magic-market-router/tests/intelligence_routing.rs`

- [x] Add optional source-semantic fields without guessing missing values.
- [x] Preserve checked deserialization invariants for coupled optional fields.
- [x] Cover serde bypass attempts and Router evidence/forwarding behavior.
- [x] Pass Core, Router and analysis tests plus strict Clippy on the active
  default toolchain, recording the actual compiler version.

Required field additions:

- `EarningsEstimate`: contributor count, EPS minimum and EPS maximum.
- `ResearchReport`: optional industry code/name.
- `Announcement`: optional category.
- `InvestorQuestion`: optional source question id and answerer.
- `DragonTigerSeat`: optional buy/sell/net values while preserving source side.
- `PopularityRank`: rank change, return ratio, heat, concepts and tag.
- `BoardFlow`: tier flows and leader identity.
- `MarginBalance`: repayment and securities-lending quantities.
- `BlockTrade`: close price and premium ratio.
- `HolderCount`: absolute holder change and average shares per holder.
- `LockupEvent`: able shares and free-float ratio.
- `DividendPlan`: ex-dividend date.
- `LimitPoolEntry`: board name, seal state and reseal count.

## Task 2: Add isolated Provider crate scaffolds

**Files:**

- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/magic-eastmoney-rs/`
- Create: `crates/magic-cninfo-rs/`
- Create: `crates/magic-ths-rs/`
- Create: `crates/magic-cls-rs/`
- Create: `crates/magic-baidu-rs/`
- Create: `crates/magic-iwencai-rs/`

- [ ] Give every crate a typed error, injected fixture transport and real
  bounded HTTPS transport.
- [ ] Enforce a strict hostname allowlist, zero redirects, positive timeout and
  response-byte cap before parsing.
- [ ] Provide accurate capability declarations and explicit
  unsupported/authentication errors.
- [ ] Add compile-only skeletons without advertising unimplemented families.
- [ ] Update the lockfile offline and pass workspace check before delegation.

## Task 3: Implement Eastmoney intelligence families

**Files:**

- Create/modify: `crates/magic-eastmoney-rs/src/*.rs`
- Create/modify: `crates/magic-eastmoney-rs/examples/live_probe.rs`
- Create/modify: `crates/magic-eastmoney-rs/examples/load_probe.rs`
- Create/modify: `crates/magic-eastmoney-rs/tests/*.rs`

- [ ] Implement instrument research and industry research with canonical/PDF
  links and published-source evidence.
- [ ] Implement one-minute and historical fund-flow series.
- [ ] Implement board flows with verified tier and leader fields.
- [ ] Implement Dragon-Tiger entries and seat details.
- [ ] Implement margin, block trades, holder counts, lockups and dividends.
- [ ] Implement upper/broken/lower/previous-upper pools and source reasons.
- [ ] Implement popularity ranking and public news only when current response
  contracts are real-verified.
- [ ] Keep official Choice/EMQuant and public-web Eastmoney identities separate.
- [ ] Pace remote calls at concurrency 1 and at least one second apart in load
  probes; cap transient retries at three.

## Task 4: Implement CNInfo and Tonghuashun families

**Files:**

- Create/modify: `crates/magic-cninfo-rs/src/*.rs`
- Create/modify: `crates/magic-cninfo-rs/examples/*.rs`
- Create/modify: `crates/magic-ths-rs/src/*.rs`
- Create/modify: `crates/magic-ths-rs/examples/*.rs`

- [ ] CNInfo: explicit instrument/org mapping, bounded announcement pagination,
  canonical/PDF URLs and optional categories.
- [ ] CNInfo: bounded Interactive Easy questions, answer state, answerer and
  source identifiers.
- [ ] Tonghuashun: strong-stock reasons and themes.
- [ ] Tonghuashun: upper-limit reasons/reveal data and popularity list.
- [ ] Tonghuashun: consensus estimates with per-year contributor/range fields.
- [ ] Keep concurrency at one and minimum one-second pacing in load probes.

## Task 5: Implement CLS, Baidu and iWencai families

**Files:**

- Create/modify: `crates/magic-cls-rs/src/*.rs`
- Create/modify: `crates/magic-baidu-rs/src/*.rs`
- Create/modify: `crates/magic-iwencai-rs/src/*.rs`
- Create/modify: corresponding `examples/` and tests.

- [ ] CLS: implement the currently verified signed telegraph/global-news
  request, validate response errno and retain publisher/source time.
- [ ] Baidu: implement verified unadjusted historical K-line and
  MA5/10/20 mapping, capped at one request and 2,001 rows.
- [ ] iWencai: accept an explicit API key, cap results at 50 and return a typed
  authentication error when absent/rejected.
- [ ] Never import browser/desktop Cookies, tokens or account state.

## Task 6: Deterministic, live and load acceptance

**Files:**

- Modify: every Provider `live_probe.rs` and `load_probe.rs`
- Modify: `docs/PERFORMANCE_RESULTS.md`

- [ ] Print every implemented record field in bounded live probes.
- [ ] Reject empty/nonconforming responses instead of producing complete empty
  batches.
- [ ] Run all deterministic unit/integration tests.
- [ ] Run current real endpoint probes; label unavailable/auth-only families
  explicitly rather than claiming success.
- [ ] Run conservative load probes and record request count, concurrency,
  success/failure, records, throughput and latency percentiles.

## Task 7: Documentation, review and release

**Files:**

- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/DEPLOYMENT.md`
- Create: `docs/integrations/eastmoney-web.md`
- Create: `docs/integrations/cninfo-web.md`
- Create: `docs/integrations/tonghuashun-web.md`
- Create: `docs/integrations/cls-web.md`
- Create: `docs/integrations/baidu-web.md`
- Create: `docs/integrations/iwencai-api.md`
- Modify: `.planning/2026-07-23-a-stock-data-parity/*`

- [ ] Document endpoint class, authorization assumptions, host allowlists,
  response limits, pacing, source times, live evidence and residual gaps.
- [ ] Run format and locked all-target check/workspace tests on the active
  default toolchain.
- [ ] Run strict workspace Clippy, rustdoc, doctests, documentation links,
  compliance and release preflight.
- [ ] Complete independent code review and close every P0/P1.
- [ ] Verify the user's requirements document remains unstaged.
- [ ] Commit, push `main` and record exact commits.

## Out-of-scope authorization dependencies

These remain explicit production gaps until the user supplies an authorized
feed/SDK:

- exchange-standardized call-auction snapshots;
- post-close fund-flow Top10 with stable documented semantics;
- Level-2 ten-level book, order queue and order-by-order feed;
- private Choice/THS/iWencai products not exposed by the activated API account.
