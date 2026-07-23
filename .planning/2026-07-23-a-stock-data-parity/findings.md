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
- Tencent already parses one strict `~`-delimited snapshot per requested symbol
  and retains the full response through a shared bounded HTTPS transport.
  Market statistics can extend the private `Snapshot` without a second call.
- The current Tencent validator accepts only six-digit equities. Reference
  parity also requires index and ETF symbols, so Slice B must add an explicit
  exchange/asset/code table and exact returned-symbol validation rather than
  prefix guessing.
- Sina already has the required bounded HTTPS transport, GB18030 decoding and
  Referer support. Financial statements and options can be isolated modules
  using the same injected transport.
- Reference option behavior confirms one Sina family for contract discovery,
  one T-quote response and one Greeks/IV response. Greeks contain three empty
  positional slots that must be skipped only after exact shape validation.
- Tencent enrichment fields are positions 38/39/44/45/46/47/48/49/52.
  Turnover is percent; market caps are source `亿元` and must be multiplied by
  100,000,000 for Core CNY money; PE/PB/volume ratio are finite scalars; zero or
  blank source values need field-specific absent semantics.
- Reference Tencent code documents only an informal Shanghai-index whitelist.
  Core's explicit `Exchange` and `AssetClass` remove that ambiguity: the
  provider can require the caller to distinguish Shanghai index `000001` from
  Shenzhen equity `000001`.
- Sina option discovery uses the JSON `StockOptionService.getStockName`
  endpoint for contract months, then `OP_UP_/OP_DOWN_` list variables for codes,
  `CON_OP_` for T-quotes and `CON_SO_` for Greeks/IV.
- Sina option T-quotes require at least 43 fields; Greeks require at least 16,
  with raw positions 1..3 exactly empty before mapping raw 4 onward.
- The reference's Sina financial fix confirms the live JSON nesting is
  `result.data.report_list`, keyed by report period, with each period's `data`
  containing line items. Static top-level `lrb/fzb/llb` lookup is known wrong.
- Tencent's existing short quote fixtures stop at field 37, so base quote
  parsing must remain backward-compatible while market-statistics parsing
  separately requires the enrichment fields through index 52.
- Tencent's current shared snapshot path and validator are equity-only. Market
  statistics need a dedicated validation path for explicit `Equity`, `Index`
  and `Fund` identities rather than weakening quote/order-book validation.
- Core's initial option records preserve only a normalized subset and require
  exact expiry/strike during discovery. Sina exposes the contract month before
  exact expiry/strike and later exposes full T-quote/Greeks depth, so Slice B
  must widen the new, unreleased option contracts without fabricating values.
- Tencent and Sina both use bounded injected transports, exact cardinality
  checks and per-source provenance helpers. Slice B can preserve those safety
  properties by adding source-family modules and reusing the client transport,
  rather than introducing an unrelated HTTP stack.
- Sina's production transport currently fixes the Referer to
  `finance.sina.com.cn`. The option endpoints require the stock-finance
  origin, so the transport contract needs a defaulted per-request Referer
  method that keeps existing fixture transports source-compatible.
- Existing live probes already print every normalized base field. Slice B
  probes should extend these executables instead of introducing hidden
  one-off verification binaries.
- The only current construction sites for `OptionContract`, `OptionQuote` and
  `OptionGreeks` are Core tests; Router routes them generically. Widening these
  new records and changing the discovery filter from exact date to contract
  month therefore has a small, fully searchable compatibility surface.
- Financial lines already retain both a normalized key and source label with
  optional numeric value/unit. Sina can populate every returned row, including
  source-empty values, without converting absence to zero.
- A real 2026-07-23 Tencent response for `sh600396`, `sh000001` and `sh510050`
  confirmed all three identities use market code `1` plus the exact requested
  symbol. Equity fields 38/39/44/45/46/47/48/49/52 were populated.
- The real index response uses `-1` sentinels for upper/lower limits, while the
  ETF response leaves PE fields blank and supplies PB as explicit zero. The
  adapter must map non-positive price sentinels to absent, blanks to absent and
  retain explicit finite zero for scalar statistics.
- Current base snapshots already parse the full response while ignoring fields
  after 37. A private optional enrichment payload on `Snapshot` lets one
  response serve the new Provider without affecting existing quote behavior.
- The real Sina income response for `sh600396` confirms the corrected shape and
  additionally exposes stable `item_field` English keys, `rCurrency`,
  `publish_date`, `update_time`, `item_source` and nullable `item_value`.
  There is no need to invent keys or announcement dates.
- `report_list` is a JSON object whose iteration order is not an API ordering
  guarantee. The parser must sort normalized report periods explicitly and
  reject duplicate/invalid dates, empty item keys/labels and non-finite values.
- Existing Sina source modules share `now`, exact instrument validation and the
  injected transport from `lib.rs`; a new `financials.rs` can follow the same
  pattern and return normalized `DataBatch<FinancialStatement>` directly.
- Reference v3.5.0 intentionally requests eight statement periods by default;
  matching that bound is sufficient parity and keeps the existing 1 MiB
  response limit meaningful.
- A live balance-sheet response reported status code 0, 102 total available
  periods and returned the requested two. Each period had 141 display rows.
  Structural section headings can have both `item_field` and `item_value`
  empty; they are not financial facts and must be skipped rather than assigned
  fabricated keys.
- The source `report_count` is the total history count, not the returned-page
  cardinality, so it must not be compared to `report_list.len()`.
- The real cash-flow response uses the same status/currency/publish metadata
  and exposes stable keys such as `LABORGETCASH`; statement-kind parsing can
  share one strict response implementation.
- The live 8-period balance response repeats `DOMETICKSETT` twice per period
  with identical label, null value, group and display type. This is a source
  duplicate, not two distinguishable facts. Retain one normalized copy and
  emit an explicit quality issue; continue to reject conflicting duplicates.
- Parallel Sina option implementation confirmed the 2026-07-23 live shapes:
  discovery repeats the nearest month as a marker, T-quotes have 51 fields and
  Greeks have 17 raw fields with positions 1..3 exactly empty.
- Core `Money` deliberately permits signed fund-flow values, so each Provider
  amount field must enforce its own semantics. Sina option turnover amount is
  non-negative and needs an adapter-level check.
- Sina T-quote source time is `YYYY-MM-DD HH:MM:SS`; record evidence should
  validate and normalize it to `YYYY-MM-DDTHH:MM:SS+08:00`, matching the rest
  of the workspace.
- The full real Sina probe passed after the duplicate-source fix and option
  wiring. Documentation still labels Tencent statistics and Sina statements/
  options as pending or unsupported in the top status, intelligence matrix,
  Provider matrix, deployment probe descriptions and integration contracts;
  Slice B cannot ship until all of these statements are reconciled.

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
| The first Slice B planning append targeted a heading that does not exist | Inspected the actual planning tail and patched the existing audit section without losing prior findings. |
| Task 1's first post-edit format check found only import wrapping | Applied rustfmt and confirmed Core/Router strict Clippy with `-D warnings`. |
## 2026-07-23 Slice B documentation decisions

- README and Tencent integration documentation now describe the implemented Tencent `MarketStatisticsProvider`, its source fields, units, live probe, and measured load result.
- Sina documentation must cover three independent public-source families: base quote/K-line/minute data, financial statements (`fzb`/`lrb`/`llb`), and ETF option discovery/T-quote/Greeks.
- Sina firewall documentation must include both `hq.sinajs.cn` and `stock.finance.sina.com.cn`; financial reports use `quotes.sina.cn`.
- Performance documentation must distinguish the earlier Tencent four-operation mixed run from the new current five-operation rotation, and record dedicated statistics/financial/options measurements without retroactively changing historical evidence.
- Sina load probe now accepts `quotes`, `bars`, `minute`, `financial`, `options`, and `mixed`; it enforces 40-request/4-worker limits.
- Sina option safety limits are 12 months, 256 contracts per call/put list, 4,096 discovered contracts, 50 contracts per quote/Greek batch, and 128 decoded fields.
- Sina financial statement requests accept at most 10 instruments. Router source adapter names must be checked from the Core tree before documenting them; the first lookup used an obsolete router file path.
- `docs/PERFORMANCE_RESULTS.md` still describes Tencent `mixed` as four families even though current code has five; historical 100/8 evidence must remain labeled as the pre-statistics rotation.
- Deployment currently describes Sina only as Quote/K-line/minute and opens two Sina hosts. It must add financial/options capabilities, `stock.finance.sina.com.cn`, and updated health/load operations without changing the seven packaged probe binary count.
- Changelog already contains the Core intelligence-contract/analysis slice, but not the implemented Tencent statistics or Sina statements/options.
- Two full Rust 1.83 checks reused a stale `magic_market_core` artifact: Cargo checked only `magic-sina-rs` and reported the pre-widening option API even though the source exports `ContractMonth` and the new fields. Manifests and `cargo metadata` confirm Sina points to the local Core path. The next diagnostic must use a fresh isolated target directory to distinguish stale incremental metadata from source defects.

## 2026-07-23 independent Slice B review

- No P0 findings; ten P1 findings block commit.
- Tencent base Quote/OrderBook/metadata parsing currently invokes extended statistics parsing and can fail on enrichment-only anomalies; these paths must be isolated and tested.
- Tencent statistics must not return a strict complete batch when every statistic is absent.
- Core option records need checked deserialization for cross-field month/date, money, book, OHLC/limit, timestamp, and Greek-domain invariants.
- Sina option output documents five levels but currently retains only top-of-book; either implement the five verified levels or narrow the contract/documentation. Completeness target favors implementation.
- Sina Greeks need domain/range checks; contract-list and quote field-count caps must be separated.
- Only 510050 has live evidence; 510300/588000/510500 must be probed before being called live-verified, otherwise documentation must label them implemented/unverified.
- Docs use the wrong sample-count environment variable; load options has a stale hard-coded contract and must discover or require current contracts deterministically.
- Router option adapters lack forwarding/evidence tests.
- Tencent changelog falsely mentions a 52-week range that is not represented by Core.
- Core option structs currently derive `Deserialize` directly and have no constructors; checked deserialization should use private raw DTOs plus shared `validate()` methods so source adapters and serde enforce identical cross-field invariants.
- Sina option decoding uses one global 128-field cap before determining record type, which makes the documented 256-code list cap unreachable. Split the decoder cap from per-record semantic caps (or raise the shell cap to cover list records while quote/Greek parsers enforce their exact maxima).
- The reference project's actual `sina_option_tquote` function maps only top bid/ask plus summary fields; its changelog claim of five levels is not backed by that function. The raw 12..31 fields look like ten price/quantity pairs, but the fixture's apparent ask-one quantity does not equal summary `ask_vol`, so field semantics need external/source verification before standardizing them as a five-level book.
- Core already has checked `BookLevel`, but adding option five-level arrays without verified source ordering would violate the no-fabrication rule. Until verified, accurately document top-of-book rather than treating the reference changelog as a field contract.
- Core `SourceEvidence` validates non-empty timestamp text but not ISO calendar semantics, so `OptionQuote` needs its own checked `quote_at` validator. The project already has private date/time validators in `provider.rs`; options can implement an equivalent strict `YYYY-MM-DDTHH:MM:SS+HH:MM` calendar check without exposing unrelated provider internals.
- Core `Money` is intentionally signed for net-flow records, so option quote validation must explicitly require non-negative amount at the record layer.
- `IsoDate` exposes checked `as_str()`, allowing exact `OptionContract.expiry` month matching without duplicating calendar logic. `quote_at` uses a full timestamp rather than `IsoDate`, so only its date prefix can reuse `IsoDate`; clock and UTC-offset syntax need local validation.
- Probe/docs lane removed stale option contracts, unified `MAGIC_SINA_OPTION_SAMPLE_CONTRACTS`, added one-time current-contract discovery, removed the Tencent 52-week claim, and correctly separated 510050 live evidence from three implemented/unverified ETF underlyings.
- Follow-up doc audit found two residual wording issues: `OptionCapabilities` uses `quotes`, not `t_quotes`; Sina option response limits are now a 257-field envelope with semantic caps of 64 quote fields and 32 Greek fields, not a universal 128.
- Second independent review closed the original ten findings but found a Unicode panic path in fixed-byte `quote_at` slicing, a Sina direct-construction gap for zero/half book levels and negative amplitude, and an omitted Sina option host in README.
- The timestamp validator now rejects non-ASCII before slicing and has a `catch_unwind` regression. Sina atomically normalizes zero-price/zero-quantity top levels, rejects half levels/negative amplitude, and its normalized record must round-trip through Core serde. README now lists all three Sina hosts.
- Final Slice B scope is 24 tracked modified files plus two intended new Sina modules (`financials.rs`, `options.rs`), with 2,048 tracked insertions and 145 deletions before adding the new-file line count. `git diff --check` passes. The only unrelated untracked file is the user's requirements document and remains excluded.
- Final staged scope is 26 files with 3,533 insertions and 145 deletions, including the two intended new Sina modules. Cached diff whitespace passes; `docs/integrations/stock-analysis-market-data-requirements.md` is the sole unstaged/untracked file.
