# Findings: Official Macro, Global Data, SEC, and Financial News

## Existing Contract Coverage

- `magic-market-core::calendar` models future or announced economic events,
  not historical macroeconomic observations.
- `magic-market-core::global` models a small closed set of current global-index
  and FX snapshots. It does not model economic time series, benchmark-rate
  histories, or official FX fixings.
- `magic-market-core::content::NewsItem` already supports the required
  metadata-only news boundary with provider, source time, observation time,
  and batch evidence.
- No Core filing contract currently represents SEC CIK, accession number,
  form, filing date, report period, and canonical document URL together.
- The current workspace has no NBS, PBC, CFETS, FRED, IMF, World Bank, SEC,
  Xinhua Finance, Yicai, or Securities Times Provider identity.

## Source Scope

- China official data: National Bureau of Statistics, People's Bank of China,
  and CFETS/China Money.
- Global public data: FRED, IMF, and World Bank.
- Company filing metadata: SEC EDGAR public data.
- Financial-news discovery: public first-party metadata from Xinhua Finance,
  Yicai, and Securities Times only when a bounded live audit proves a stable,
  authorized endpoint.
- Tushare paid data, Wind, Choice, iFinD, Bloomberg, licensed research bodies,
  broker-account data, and logged-in browser extraction are excluded from this
  first batch.

## Evidence Boundaries

- A local fetch time is observation evidence, never a fabricated source
  release time.
- Missing numeric observations remain explicit source states and are never
  converted to zero.
- Provider-native indicator codes remain provider-scoped. Similar names from
  two institutions are not silently treated as the same series.
- Pagination is atomic. Partial pages, duplicate identities, contradictory
  metadata, unit changes, and response revisions during one fetch fail the
  batch.
- Public-news implementations expose only title, provider-native or canonical
  identity, canonical URL, publisher, language, topics when supplied, and
  source publication time. Summary and content remain absent.

## Official Protocol Research

- FRED's documented v1 series path is
  `https://api.stlouisfed.org/fred/series/observations`; JSON requires
  `series_id`, `api_key`, and `file_type=json`. It supports observation date
  bounds, pagination, realtime/vintage fields, and reports missing values as
  `"."`. The API key is mandatory and subject to FRED terms.
- IMF DataMapper documents API v2 for indicator/country/region/group time
  series with an optional `periods` query. Dataset namespace must remain part
  of series identity because the public catalog spans multiple datasets.
- The IMF help page's exact official v2 base is
  `https://www.imf.org/external/datamapper/api/v2/`. Catalog routes are
  `indicators`, `countries`, `regions`, and `groups`; a series request appends
  the indicator and zero or more geographic IDs, and `periods` is a
  comma-separated year list. The page's canonical examples use
  `NGDP_RPCH/USA/CHN`.
- A narrow 2026-07-29 live audit of that exact IMF example route returned the
  requested indicator but also a full geographic/year superset despite the
  path IDs and `periods` query. The Provider may validate and filter a bounded
  complete official response, but cannot claim server-side geographic or
  period filtering. Admission must fail if response size/shape exceeds the
  fixed full-envelope bounds.
- IMF indicator metadata for `NGDP_RPCH` includes exact label, description,
  source release, unit, dataset (`WEO`), projection year, and last-modified
  time. Dataset namespace, unit, and projection/revision evidence can
  therefore be validated against the catalog before mapping values.
- A World Bank v2 audit confirmed the two-element response: page metadata
  followed by rows containing indicator ID/name, country source ID/name,
  ISO-3 code, year, nullable value, `unit`, observation status, and decimal
  precision. The separate indicator endpoint supplies source ID/name, note,
  organization, and topics.
- The audited World Bank GDP indicator returned an empty `unit` field even
  though its source name/note describes current US dollars. The Provider must
  not infer a unit from prose. Generic admission requires an indicator whose
  official structured `unit` is non-empty, or the affected request must fail
  with an explicit protocol error.
- A bounded catalog audit of the first 20,000 of 29,544 World Bank indicators
  found zero non-empty structured `unit` fields. The first implementation can
  still provide a strict client/parser, but `ECONOMIC_SERIES_ADMITTED` must
  remain false under the approved mandatory-unit Core contract unless a
  separately documented official structured unit source is proven.
- Because the approved Core request has no separate region selector,
  geographic series identities use the provider-native namespace rather than
  inventing a new request field: examples are `WEO/USA` plus IMF indicator
  code and `source:2/country:USA` plus World Bank indicator code. The
  observation still carries the exact source region code/name separately.
- World Bank's public v2 Indicators API is anonymous. The official country /
  indicator form is `https://api.worldbank.org/v2/country/{economies}/indicator/{indicators}`;
  JSON uses `format=json`, paging uses `page` and `per_page`, and the official
  documentation caps a request at 60 indicators.
- SEC's public submissions endpoint is
  `https://data.sec.gov/submissions/CIK##########.json`. It is anonymous,
  returns at least one year or 1,000 recent filings plus named older
  submission files, and is updated throughout the day.
- SEC public archive links use `https://www.sec.gov/Archives/edgar/data/...`.
  The public data API is distinct from authenticated filer-submission APIs.
  Automated access must identify the application through `User-Agent` and
  follow SEC fair-access policy.
- CFETS states that it officially publishes RMB central parity, Shibor, LPR,
  DR, and related benchmarks. Its member data-interface services are a
  separate product boundary and cannot be assumed public merely because
  benchmark pages are public.
- Direct status checks proved the public Shibor page and LPR historical
  download page return HTTP 200 as HTML. The originally guessed RMB parity
  path returned 404; official search identifies the live page as
  `https://www.chinamoney.com.cn/chinese/bkccpr/index.html?tab=2`.
- CFETS documents 24 central-parity currencies and their distinct history
  start dates. Quotation direction differs by currency, so the Provider must
  preserve exact base/quote orientation and quotation base rather than
  assuming every row is foreign-currency-per-CNY.
- The public CFETS pages load first-party history fragments from
  `/r/cms/chinese/chinamoney/html/shibor/shibor-his-cn.html`,
  `/r/cms/chinese/chinamoney/html/shibor/shibor-quote-his-cn.html`,
  `/r/cms/chinese/chinamoney/html/shibor/shibor-mn-his-cn.html`, and
  `/r/cms/chinese/chinamoney/html/fx/ccpr-his.html`.
- The audited public LPR history fragment sends a JSON `POST` to the
  first-party `cm-u-bk-currency/LprHis?lang=CN` route, with
  `strStartDate`/`strEndDate`, and expects dated columns/records. Its separate
  spreadsheet export is not required by this phase.
- The audited Shibor fragment sends a JSON `POST` to
  `cm-u-bk-shibor/ShiborHis`, optionally with `lang=cn`, `startDate`, and
  `endDate`. The response binds `data.baseCurveCfgList` tenor definitions to
  complete `records`; the parser must reject a tenor/header mismatch instead
  of accepting positional drift.
- The audited central-parity fragment calls
  `cm-u-bk-ccpr/CcprHisNew` with `startDate`, `endDate`, `currency`,
  `pageNum`, and `pageSize`. The response supplies the currency headings,
  records with a date plus positional values, total/page metadata, and the
  source-supported date window. All pages must retain identical headings and
  bounds before an atomic batch is admitted.
- For filtered central-parity queries, `data.head` remains the complete source
  catalog while `data.currency` (comma separated) and `data.searchlist`
  identify the selected columns; `records[].values` aligns to that selected
  order, not the complete `head`. The Provider must validate both roles and
  never zip filtered values against the full catalog.
- The official page imports
  `/r/cms/chinese/chinamoney/assets/js/cm-basic.js`; that first-party script
  declares `BK_URL = "/ags/ms/"` and `DQSURL = "/dqs/rest/"`. The JSON
  Provider therefore uses same-origin `https://www.chinamoney.com.cn/ags/ms/`
  routes and does not depend on a private/member service hostname.
- Narrow live requests on 2026-07-29 proved the public response shapes:
  Shibor returns eight named tenors and dated keyed records; LPR returns `1Y`
  and `5Y`; central parity returns a complete heading list plus positional
  values, page totals, source bounds, and selected currencies. These source
  facts are suitable for frozen conformance fixtures.
- The official CFETS media-data service page says website/media republication
  requires application and authorization and forbids bulk-download or
  hotlink-style acquisition. The crate must document operator responsibility,
  expose only bounded on-demand retrieval, avoid mirroring or background bulk
  crawling, and must not claim that a technically public page grants
  redistribution rights.
- The official-site search did not prove a bounded historical DR007 page/API
  equivalent to the Shibor, LPR, and central-parity pages. DR007 therefore
  remains an explicit unsupported identity in the first CFETS admission
  rather than being substituted with R007, Shibor 1W, or a media article.
- PBC publishes individual official statistical releases such as monthly
  social-financing reports, but the initial search did not prove one stable
  structured time-series API. Planning must treat first-party publication
  pages/documents as the candidate contract and require a live layout audit.
- NBS's first-party `https://data.stats.gov.cn/easyquery.htm` exposes monthly,
  quarterly, annual, census, regional, departmental, and international query
  views. Public page URLs carry `cn`, `sj`, and `zb` identities, but the
  official site does not publish a supported machine-API contract in the
  material found. The Provider must start with a diagnostic request/parser
  and keep production capability false until the exact backing request is
  live-proven and documented.
- CFETS's first-party Shibor page documents eight tenors and an 11:00 Beijing
  publication time on each business day. Its LPR page documents one-year and
  over-five-year tenors and a monthly 09:00 publication. The official pages
  link to historical-data views; implementation must use only exact public
  history/export paths proven by audit, not member interface products.
- PBC publishes first-party PDF statistical tables with explicit units,
  periods, footnotes, and methodology breaks. A production parser must bind a
  named table family and validate every row/header/footnote; a general PDF
  text scraper cannot truthfully advertise a stable economic-series
  capability.
- PBC also publishes first-party HTML table resources below
  `/eportal/fileDir/diaochatongjisi/resource/cms/{year}/{month}/{id}.htm`.
  Audited examples expose bilingual table title, unit, monthly period headers,
  values, and footnotes as structured HTML, avoiding PDF text extraction for
  admitted table families.
- PBC social-financing release pages expose an exact article publication time,
  while separate region tables may link XLSX attachments. Production scope
  should begin with the HTML money-supply table (`M0`, `M1`, `M2`) and a
  release-page social-financing total, not a generic PDF/XLSX scraper.
- PBC methodology footnotes explicitly describe series breaks, including the
  revised M1 measurement from January 2025. These notes must feed the
  `EconomicRevision`/source-note boundary and cannot be discarded as layout
  noise.
- Official-site search found current aggregate-financing flow tables as PDF
  and regional releases as XLS/XLSX attachments, not an audited structured
  HTML/API family equivalent to the money-supply table. Under the approved
  no-generic-PDF/XLSX boundary, social-financing production admission remains
  false until a separately designed document parser or structured first-party
  contract is proven.
- Xinhua Finance has a public first-party listing at
  `https://www.cnfin.com/news/index.html`; Yicai has public first-party
  listings at `https://www.yicai.com/news/` and
  `https://www.yicai.com/news/info/`; Securities Times has public first-party
  listings at `https://www.stcn.com/article/` and category paths below
  `/article/list/`.
- Search evidence proves server-visible titles and times for those listing
  pages, but does not prove a documented RSS/JSON contract. The news plan must
  start with exact server-rendered-list metadata probes and validate canonical
  links; it must not depend on hidden application APIs.
- Yicai publishes an explicit copyright statement and provides only a
  podcast-specific RSS result in the official-site search. News records must
  remain title/link/time metadata with `summary=None` and `content=None`;
  no general Yicai RSS capability can be claimed.
- An escalated direct audit on 2026-07-29 fetched all three news listing
  documents successfully: Xinhua Finance 64,380 bytes, Yicai 344,592 bytes,
  and Securities Times 179,351 bytes.
- Yicai's public server-rendered `/news/info/` document embeds a `firstlist`
  JSON array. Each row includes `NewsID`, `NewsTitle`, exact `CreateDate`,
  `NewsSource`, and canonical relative `url`; it also includes `NewsNotes`
  content that the Provider must neither deserialize into a retained field nor
  expose. This is sufficient for a bounded first-page metadata Provider
  without calling a hidden API.
- Xinhua Finance renders canonical detail links under
  `/yw-lb/detail/YYYYMMDD/{id}_1.html` and runs an additional list script. The
  implementation may parse only the initial server-rendered rows if their
  exact times are present; client-side subscription/account endpoints are out
  of scope.
- The Securities Times home page renders canonical
  `/article/detail/{id}.html` links but many home-page blocks omit publication
  time. Its dedicated first-party fast-news list is the better candidate and
  must be audited separately before admission.
- Xinhua Finance's server-rendered news list contains exactly 13 initial
  `.ui-zxlist-item` rows in the audited response. Each row has one canonical
  detail URL, title, exact `YYYY-MM-DD HH:MM:SS` source time, and a category
  label. Summary `<p>` elements are present and must be skipped without
  retention.
- The Securities Times fast-news page intentionally renders an empty list and
  declares the first-party data path
  `/article/list.html?type=kx` through a `data-url` attribute. This disclosed
  endpoint requires a separate bounded response audit; account, notification,
  CAPTCHA, and stock-watch endpoints remain excluded.
- Yicai's listing document contains many embedded news objects and exact
  `CreateDate` fields. The parser must locate exactly one `var firstlist = ...`
  assignment, cap its JSON array before allocation, and ignore `NewsNotes`,
  image, speech, video, creator, popularity, and mobile-share fields.
- A plain GET to the Securities Times disclosed list path returned the full
  page rather than row data. This is not admissible yet; the source's public
  list script must prove the exact method, headers, parameters, response type,
  and bounds before the capability can be enabled.
- The first-party Securities Times quick-news script calls a shared
  `infinite(...)` loader and passes optional `tag`, `date`, `isRed`, and `tab`
  parameters. The list-specific script does not define request mechanics, so
  the first-party shared script must be audited before fixing an endpoint
  contract.
- The shared first-party loader uses `$.getJSON` against the page-declared URL
  with `page_time` and `last_time` cursor parameters. A successful response is
  expected to contain `state`, `page_time`, `last_time`, and a `data` array.
  The production request must reproduce only jQuery's public GET/JSON
  negotiation headers and must validate monotonic cursors and the whole
  bounded page before truncation.
- With JSON/XHR headers but explicit empty cursor strings, the endpoint
  returned `state:1`, `data:""`, and `page_time:null`. The parser must reject
  unexpected polymorphism for non-empty pages and recognize only a separately
  evidenced terminal-empty shape. Initial requests should omit undefined
  cursor keys rather than serializing them as empty strings.
- Omitting undefined cursor keys while retaining standard jQuery JSON/XHR
  headers produced a 78,723-byte JSON response with `state:1`, 30 rows,
  `page_time:2`, and a numeric `last_time`.
- Each audited Securities Times row supplies string `id`, canonical relative
  `url`/`web_url`, `title`, `source:"人民财讯"`, millisecond `time`, second
  `show_time`, and cursor `pageTime`. It also supplies full `content` and share
  descriptions that the Provider must skip without mapping or probe output.
- The Securities Times first-page Provider can safely cap its public limit at
  30 and validate all 30 rows before truncation. It does not need historical
  cursor traversal in this first metadata-only slice.

## Workspace and Baseline

- The isolated feature worktree was created from current `origin/main`.
- `cargo build --workspace --locked --offline` passed.
- `cargo test --workspace --all-targets --locked --offline` passed.

## Planning Structure

- The approved specification spans independent Core/transport, China official
  data, global macro, SEC, public news, and final integration concerns.
- Implementation plans will be split along those boundaries so each plan has
  explicit files, tests, commands, commits, and a working acceptance point.
- Core currently keeps each record family in one focused `src/*.rs` module
  with public integration tests under `crates/magic-market-core/tests/`.
- Router keeps provider-neutral adapters in `src/adapters.rs`, generic source
  machinery in `src/source.rs`/`src/router.rs`, and family-specific routing in
  focused modules where needed.
- Small Providers use `src/lib.rs` plus a transport/parser split and
  `tests/capabilities.rs`; larger Providers split mappings and source families
  into focused modules.
- New source crates should match workspace version `0.2.0`, inherit workspace
  lints, pin already locked protocol dependencies where applicable, and use an
  exact `=0.2.0` Core path dependency.
- Release registration currently spans root `Cargo.toml`, README,
  `docs/DEPLOYMENT.md`, `docs/business_rules.md`, integration docs,
  `tools/compliance/check.sh`, `tools/release/package.sh`, and preflight.
- Checked Core requests use private fields, checked constructors/accessors,
  custom `Deserialize`, typed capability structs, provider traits, and
  `SourcedRecord`; new contracts must follow the same pattern.
- The exact reusable checked primitives live in
  `magic-market-core/src/validated.rs`, evidence in `evidence.rs`, batch
  admission in `batch.rs`/`probe.rs`, provider identity and `SourcedRecord` in
  `provider.rs`, and errors in `error.rs`; the foundation plan must change and
  test those existing boundaries instead of duplicating validation inside
  source crates.
- Provider identity tests already assert stable JSON names and are the correct
  red-test entry point for the ten new identities.
- Router family adapters validate request cardinality, identities, ordering,
  and source facts before allowing generic failover. New family adapters
  belong in a focused macro/filings module rather than enlarging unrelated
  intelligence code.
- Existing family adapters are closure-backed `SourceFn` constructors that
  call the Provider trait, classify its typed error, then validate exact
  requested identities/order before returning the batch. The new macro,
  reference, fixing, and filing adapters should follow this shape in focused
  `macro_data.rs` and `filings.rs` Router modules.
- `global_news_source` already caps output by the requested limit and rejects
  duplicate item IDs; the three new news Providers need no new Router family,
  only evidence-preserving identity coverage in
  `tests/intelligence_routing.rs`.
- Release documentation has canonical files `docs/UPSTREAM.md`,
  `docs/DEPLOYMENT.md`, `docs/business_rules.md`, one file per source in
  `docs/integrations/`, and evidence artifacts under `docs/evidence/`.
- Core date ranges conventionally use checked `IsoDate`, compare start/end in
  constructors, cap request cardinality, and rebuild through custom
  `Deserialize`. New records should implement all four `SourcedRecord`
  evidence methods where source and observation times exist so Router
  freshness checks do not silently fall back or reject valid macro batches.
- The generic Router rejects empty batches unless explicitly opted in,
  incomplete batches under strict policy, provider/batch evidence mismatches,
  missing source times, and stale/malformed evidence. Family adapters only
  need to prove requested identities, uniqueness, range/order, and
  provider-specific metadata before the generic chain applies those checks.
- Existing public Providers commonly hold a `MutexGuard` across sleep and the
  full transport call. The new shared transport must use a reservation value
  returned after releasing the lock, with a deterministic concurrency test
  proving a second caller can reserve without waiting for the first caller's
  network completion.
- Root workspace registration is an explicit `members` array and Provider
  crates use workspace edition/license/lints, exact Core version `=0.2.0`,
  pinned protocol dependencies, and `serde_json` fixtures. The shared
  transport crate and ten source crates must be registered before a
  package-specific test can become a working checkpoint.
- Release preflight already runs format, all-target/all-feature check and
  tests, strict Clippy, Rustdoc, doctests, documentation links, compliance,
  and diff checks, with optional required coverage evidence. Packaging builds
  an enumerated set of Provider probes, so each admitted source needs explicit
  probe registration rather than relying on workspace compilation alone.
- Compliance duplicates workspace membership in `required` and
  `workspace_members` arrays and explicitly forbids concrete Provider
  dependencies in the Router manifest. Integration must register every new
  manifest/directory there and extend the forbidden dependency pattern while
  keeping Router dependent only on Core.
- `tools/release/package.sh` explicitly builds each live/load example and then
  packages tracked documentation. The release plan must add all implemented
  probe binaries, while diagnostic-only sources may package a live diagnostic
  but must retain their false capability labels.
- Registered business rules are contiguous through `BR-038` even though their
  document order is historical rather than numeric. The four new family rules
  should be `BR-039` through `BR-042`, and compliance should advance its
  contiguous check to 42.
- Baseline `bash tools/compliance/check.sh` still passes before the new
  registrations. README currently duplicates crate tables, dependency
  diagrams, status prose, probe commands, and package layout; integration must
  update every occurrence instead of only the top matrix.
- Existing Provider errors expose stable categories. The shared transport
  should return its own typed error, and each Provider should preserve
  source-specific decode/protocol/unsupported/core categories without string
  matching.
- The current lockfile already contains `quick-xml`, `regex`, `time`, `ureq`,
  and `url`, but not a DOM/HTML parser, spreadsheet reader, or PDF extractor.
  New HTML parsers should follow existing strict marker/tag scanning and avoid
  adding a broad browser/DOM dependency unless source fixtures prove it is
  necessary.
- Existing probes use Core `ProbeRequestTracker`, `LoadProbeSnapshot`,
  `verify_admitted_batch`, and `VerifiedEmpty`; new Provider probes should
  reuse these rather than invent source-local admission states.
- Historical plan documents vary from completed checklists to broad task
  summaries. This implementation set must use the stricter current planning
  standard: red/green commands, exact paths, concrete code shapes, explicit
  commits, and no `TODO`/`TBD` placeholders.
- Existing Chinese news Providers normalize source Beijing wall times to RFC
  3339 with explicit `+08:00`; the Xinhua/Yicai/STCN plans now follow that
  evidence convention so Router freshness parsing remains valid.
- Existing coverage commands and CI use `cargo-llvm-cov 0.8.7`, workspace
  all-features locked/offline collection, overall 80%, and critical aggregate
  95%. New transport/Provider source globs must join that critical set rather
  than relying only on overall coverage.
- Core capability structs must remain provider-neutral:
  `EconomicDataCapabilities` uses generic `economic_series` plus
  `regional_series`, while `ReferenceDataCapabilities` uses
  `benchmark_rates` plus `official_fx_fixings`. CFETS keeps Shibor/LPR/DR007
  detail in its own source-specific capability struct.
- New checked records must require their optional release/acceptance field to
  equal `SourceEvidence::source_at` and require provider-scoped series/rate/FX
  identities to equal the evidence Provider. This prevents a valid-looking
  public field from contradicting Router freshness/evidence checks.
- The lockfile contains `url 2.5.4`, so the transport foundation can pin that
  already-resolved version and preserve locked/offline checkpoints.
