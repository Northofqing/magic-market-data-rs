# Findings and decisions

## User requirements

- Use `https://github.com/simonlin1212/a-stock-data` as the reference project.
- Develop the complete useful capability set in the existing Rust architecture.
- Continue autonomously without confirmation pauses.

## Known local baseline

- Existing providers: TDX, Tencent, Sina and Choice/EMQuant.
- Existing normalized families: Quote, Bar, MinutePoint, Trade, MoneyFlow,
  OrderBook, AuctionSnapshot and SecurityMetadata.
- TDX covers the broadest low-level market/fund/finance/F10/XDXR surface.
- Research, news, announcements, limit-up pools, options and sentiment do not
  yet have normalized Core contracts.
- Choice currently live-verifies daily bars and daily money flow; quote,
  Level-2 and minute history are still entitlement-gated.

## Research findings

- The reference README currently advertises version 3.5.0, ten layers,
  44 distinct endpoints and 15 data sources. It is primarily one large
  Markdown skill containing executable Python recipes, not a typed service
  library.
- The advertised layers are行情、研报、信号、资金面/筹码、新闻、基础数据、公告、
  打板、ETF期权 and 舆情互动.
- The source set includes mootdx, Tencent, Baidu, Eastmoney reportapi/push2/
  push2ex/datacenter/search/news, 10jqka, iwencai, CNInfo, CLS, Sina and
  exchange/HKEX fallbacks.
- Only iwencai is documented as requiring an API key. The other public-web
  sources still need independent verification for current availability,
  anti-bot behavior, licensing and audit timestamps.
- The reference itself records several historically broken or silently wrong
  routes. This reinforces that every Rust capability must have fixtures plus a
  real acceptance probe rather than being marked complete from copied code.
- A shallow audit copy was cloned to
  `/private/tmp/a-stock-data.ooJV1X/reference`; it is research-only and will
  not be added to the workspace.
- The reference is Apache-2.0 licensed and names Simon Lin as copyright owner.
  If any implementation expression is copied rather than independently
  rewritten from protocol observations, Apache attribution/license obligations
  must be added to the release.
- Its shared Eastmoney helper is process-global, serial, uses a one-second
  minimum interval plus jitter, retries 429/5xx/connect failures, does not retry
  403, and reuses one HTTP session. The Rust equivalent needs a clone-shared
  provider limiter rather than per-request sleeping.
- Existing TDX coverage already supersedes the reference's mootdx helper,
  including real-data server validation and broader normalized contracts.
- Existing Tencent parsing currently stops before the reference's verified
  fields 38/39/44/45/46/47/48/49/52: turnover, PE TTM, total/float market cap,
  PB, price limits, volume ratio and static PE. Extending the Tencent contract
  is the smallest first parity slice.
- Baidu daily K lines add server-returned MA5/10/20, but those values can also
  be deterministically derived from normalized daily closes. The design must
  distinguish source MA evidence from locally derived indicators.
- Eastmoney stock/industry reports share one paginated endpoint differentiated
  by `qType`; report records include title/date/broker/rating/industry and
  three forward EPS fields. PDF URLs use `infoCode` and require validation of
  content type/magic, size bounds and caller-selected storage.
- 10jqka consensus EPS is parsed from a GBK HTML table and has no stable JSON
  schema in the reference. It needs an isolated HTML adapter and source-table
  version evidence; it must not use the reference's “first table” fallback.
- iwencai is the only credentialed source. It uses a Bearer API key plus
  per-request X-Claw headers and exposes report/news/announcement semantic
  search plus arbitrary structured query results. Secrets must remain
  environment-injected and arbitrary rows cannot be forced into unrelated
  typed contracts.
- The 10jqka strong-stock endpoint in the reference uses plaintext `http://`.
  The existing architecture requires HTTPS, so it cannot be admitted until an
  HTTPS route is independently proved.
- The documented northbound route explicitly says Shenzhen data is unreliable
  after disclosure changes. A local CSV accumulated from partial web data is
  not authoritative provenance; this family should use an official HKEX daily
  contract or remain explicitly degraded.
- Eastmoney `slist` returns mixed industry/concept/region memberships without a
  reliable type field. Normalization must preserve an unknown/mixed category
  rather than infer type from display names.
- Minute fund flow is a time series, while Core `MoneyFlow` is one snapshot.
  Add a separate dated/time-series request and record instead of overloading
  the existing trait.
- The reference's individual dragon-tiger implementation can read uninitialized
  `buy_data`/`sell_data` when no listing record exists, and many parsers replace
  missing numeric values with zero. Both patterns are rejected by current Core
  rules and will be rewritten with explicit absence.
- Industry ranking, board money flow and market-wide dragon-tiger are naturally
  market/board-scoped contracts rather than instrument-scoped quote metadata.
- Eastmoney datacenter families share transport but require distinct normalized
  records: margin balances, block trades, holder counts, dividend plans,
  lockups and dragon-tiger seats. The reference silently emits zero for many
  missing fields; Rust must use `Option` plus quality evidence.
- Existing TDX finance/F10/XDXR capabilities already cover part of the
  reference's basic-data and dividend surface. Provider parity should extend
  normalized contracts and cross-source validation rather than duplicate TDX
  access functions.
- Daily and minute Eastmoney fund flows share field semantics but have different
  time granularity. A common `FundFlowPoint` with explicit interval/date/time
  can support both without mislabeling daily records as realtime snapshots.
- News needs a shared `NewsItem` contract with title, summary/content, publisher,
  canonical URL, source publication time and observed time. Eastmoney stock
  news is JSONP; CLS uses a locally reproducible SHA1→MD5 request signature;
  Eastmoney global news is plain JSON.
- CNInfo announcement search requires a stock→orgId mapping and POST form data.
  The reference fetches that mapping over plaintext HTTP and falls back to
  guessed IDs; both violate current correctness/security rules. Rust must prove
  HTTPS mapping and fail explicitly when an orgId is unknown.
- Limit-up/down/broken/yesterday pools use four push2ex routes with a common
  embedded `ut` value and source-scaled prices. The `ut` value is protocol
  metadata, not a secret, but must be configurable/versioned because it may
  change.
- Limit-up sentiment is a deterministic derived aggregate and should live above
  raw pool contracts, with divide-by-zero and missing-field semantics tested.
- Sina ETF options are a distinct instrument domain: contract discovery,
  T-quote and Greeks/IV use different response families. They should extend the
  Sina provider through typed option identifiers/records rather than weaken the
  existing equity-only `SinaClient` validation.
- Option Greeks responses contain three deliberate empty slots before numeric
  fields; exact shape/version tests are mandatory because a positional shift
  silently corrupts Delta/IV.
- CNInfo IRM is a two-call flow: keyword lookup for `orgId`, then an empty-body
  POST whose filters are query parameters. Questions may legitimately have no
  answer and must retain that absence.
- 10jqka and Eastmoney popularity lists require separate ranking records;
  Eastmoney's first response contains only prefixed codes and needs a second
  batch quote lookup for names/prices. The Rust implementation must keep the
  two source/observation times rather than present the join as atomic.
- The reference's fallback sample disables TLS certificate and hostname
  validation. That is prohibited in the current architecture; every provider
  must keep normal TLS verification even when an official endpoint is used.
- The reference directly calls Eastmoney outside its limiter in the popularity
  functions. A Rust Eastmoney transport must centralize all domains and methods
  behind one shared rate/concurrency budget.
- Forward PE and PEG are neutral deterministic calculations. “PE digestion to
  30x” embeds a subjective fixed valuation anchor and belongs in an optional
  analytics policy above source data, not in Core market-data truth.
- The reference's “full valuation” is a non-atomic composition of Tencent price
  and 10jqka consensus EPS. A Rust aggregate must preserve each component's
  provenance and report partial failure rather than collapse both into one
  apparent source.
- The changelog confirms repeated silent schema bugs across K-line parameters,
  EPS column positions, report JSON nesting, announcement org IDs and source
  availability. Compatibility therefore requires behavioral results, not
  source-code parity.
- The local workspace has six crates and keeps all provider-neutral records in
  `magic-market-core::provider`, while `magic-market-router` offers one generic
  first-acceptable-batch chain plus a typed adapter per data family. New
  families can follow this exact pattern without concrete provider dependencies.
- The user's preserved requirements document deliberately excluded research,
  financial statements, announcements and news from the earlier market-data
  handoff. The newer explicit parity request broadens scope, but those business
  domains should remain separate modules/contracts so existing P0 market-data
  semantics do not change.
- Core currently supports `Equity`, `Index`, `Fund` and `Bond` asset classes.
  Options require an explicit `Option` identity or a separate contract key that
  can represent underlying, expiry, call/put, strike and source contract code.
- Existing normalized data is concentrated in one large `provider.rs`. Adding
  dozens of unrelated records there would reduce isolation; the expansion
  should split Core by business domain while re-exporting public types for
  compatibility.
- The preserved external requirements continue to require 5-second quote
  freshness, call-auction data and post-close Top10 money flow. The reference
  project does not actually solve those strict production gaps, so parity with
  it cannot be treated as completion of the separate P0 handoff.
- `ProviderId` currently has TDX, Tencent, Eastmoney, Sina, Baostock,
  LocalTerminal and Custom. Baidu, 10jqka, iwencai, CNInfo, CLS, SSE/SZSE and
  HKEX need explicit identities so routing and provenance never collapse them
  into `Custom`.
- The current flat `Capabilities` struct describes eleven market-data flags.
  Appending dozens of intelligence flags would be unwieldy; domain-specific
  capability structs should coexist with the current struct.
- Router acceptance is generic over any `SourcedRecord`; only thin adapter
  aliases/functions are family-specific. New records can reuse routing,
  evidence validation and ordered attempt traces with minimal router changes.
- Core constructors already revalidate serde input, text, dates, finite values,
  status completeness and record/batch identity. New modules must preserve that
  checked-constructor/`TryFrom<Wire>` pattern.
- The reference endpoint inventory contains two plaintext-only examples:
  CNInfo's stock mapping and 10jqka's strong-stock event route. Both need HTTPS
  verification or an explicit unsupported boundary; TLS must never be disabled.
- Eastmoney uses at least seven distinct hosts but one shared risk surface:
  datacenter-web, reportapi, pdf.dfcfw, push2, push2his, push2ex,
  search-api-web/np-weblist/emappdata. One shared limiter must cover all of
  them, including POST popularity calls.
- Eastmoney datacenter report names in current reference code are
  `RPT_DAILYBILLBOARD_DETAILSNEW`, `RPT_BILLBOARD_DAILYDETAILSBUY`,
  `RPT_BILLBOARD_DAILYDETAILSSELL`, `RPT_LIFT_STAGE`,
  `RPT_DATA_BLOCKTRADE`, `RPT_HOLDERNUMLATEST` and
  `RPT_SHAREBONUS_DET`; margin uses `RPTA_WEB_RZRQ_GGMX`.
- The current release package script only stages seven existing probes. Every
  newly advertised provider/domain needs a fixture test and a live probe; the
  package should include provider-specific probes plus one aggregate full-stack
  probe.
- `tools/compliance/check.sh` currently hard-codes the six existing workspace
  members. Adding provider or analysis crates requires updating that allowlist
  and its tests in the same change.
- Official SSE/SZSE/HKEX fallbacks belong behind explicit exchange provider
  identities and capabilities. They must not be advertised until HTTPS
  behavior, schema parsing, paging and real responses are verified.
- The reference remains a behavior/schema research source only. The Rust
  implementation will not copy its Python code or unsafe fallback behavior.
- The repository already keeps dated specifications and implementation plans
  under `docs/superpowers/specs` and `docs/superpowers/plans`; the parity design
  and each execution slice will use the same convention.
- The reviewed design is 552 lines, contains no TODO/TBD placeholders, makes
  advertised capability evidence explicit, and keeps plaintext/TLS exceptions
  outside the accepted architecture.
- The current working tree still contains only parity planning/specification
  changes plus the user's preserved untracked requirements document; no
  pre-existing code change was overwritten.
- Workspace crates live under `crates/`; the first audit path without that
  prefix was corrected before any source edit.
- Core already has a reusable `SourcedRecord` contract requiring provider and
  batch identity, while the router is generic over any such record. New domain
  records can therefore use a shared checked evidence object and avoid changes
  to the failover engine itself.
- `InstrumentId` and `Provenance` already use constructor-validated custom
  deserialization. New identities/evidence should follow this style.
- Existing `Capabilities` remains market-data-specific. New domain capability
  structs can be placed beside new traits without breaking current providers.
- Router `adapters.rs` is intentionally thin: each normalized trait maps to a
  `SourceFn` plus one router alias. New families should preserve this pattern.
- Existing value wrappers (`Price`, `Quantity`, `Money`, `Ratio`) validate
  deserialization themselves. Slice A will extend this pattern with reusable
  checked text/date/URL/finite/rank/evidence primitives so dozens of domain
  records do not each need fragile handwritten serde wire mirrors.
- Core currently has only `serde` and `thiserror`; the foundation can remain
  dependency-light. URL/date validation should be deliberately strict and
  structural without introducing a network or wall-clock dependency.
- Existing implementation plans use checkbox tasks with exact files, commands
  and acceptance assertions. The new Slice A plan will follow the same format.
- `Bar` exposes validated interval/start/end/close and batch/provider evidence,
  which is sufficient for a network-free moving-average implementation without
  reaching into provider payloads.
- `DataBatch` retains provenance and completeness independently from record
  evidence; analysis results that combine providers should keep the original
  `SourceEvidence` inputs rather than manufacture one upstream batch.
- Slice A adds a library-only analysis crate, so the release package needs no
  new executable yet. It will be included transitively when future aggregate
  probes are built.
- Root documentation currently describes only the eight low-level market-data
  families. It needs a separate normalized intelligence-domain matrix whose
  status is explicitly “contracts implemented, Provider live connection
  pending.”
- The compliance script matches the workspace member line exactly and must add
  `magic-market-analysis`; otherwise a correct workspace expansion fails the
  structural gate.
- Submission review found two important contract issues before commit:
  `TechnicalBar::new` could pair a source bar with mismatched outer
  provider/batch evidence, and `FundFlowRequest` exposed an unbounded public
  limit with unchecked derived deserialization.
- The review scan found no TODO/TBD placeholders, insecure-TLS switches or new
  plaintext endpoint use in the Slice A production code.

## Technical decisions

| Decision | Rationale |
| --- | --- |
| No direct Python runtime dependency in production Rust | The current workspace ships native Rust libraries/probes and uses subprocess isolation only for vendor SDK ABI boundaries. |
| Source-aligned provider crates with domain-aligned Core modules | Preserves the existing dependency direction while allowing one source transport/rate budget to serve several business domains. |

## Resources

- Reference: https://github.com/simonlin1212/a-stock-data
- Reference audit clone: `/private/tmp/a-stock-data.ooJV1X/reference`
- Local capability matrix: `README.md`
- TDX matrix: `docs/TDX_CAPABILITIES.md`

## Issues encountered

| Issue | Resolution |
| --- | --- |
| A combined audit command tried to read reference `SKILL.md` from the local workspace and stopped before the release-script reads | Logged the path mix-up and split subsequent reads by their correct working directories. |
