# Business rules
## BR-038 Magic TDX historical-bar cardinality error contract
Every normalized Magic TDX historical-bar path must report an exact-page
cardinality mismatch as the structured `HistoricalBarCardinality` error. The
error binds the total normalized request, current wire offset, exact current
page limit and actual decoded row count. Callers may derive the available
cardinality only with checked `offset + actual` arithmetic and only when the
error's total request equals the rejected request. Callers must not parse
display text to recover those fields. A short, empty or oversized page rejects
the complete normalized request before provenance or `DataBatch` creation;
completed earlier pages remain transport audit evidence, not successful
provider output.

## BR-036 Magic TDX normalized historical-bar exact pagination
The normalized Magic TDX `HistoricalBars` operation honors the full positive
`BarsRequest.limit` `u16` domain while every wire request remains at or below
`MAX_KLINE_COUNT=800`. Pages use exact offsets from the newest page toward
older history. Every page must succeed with its exact requested cardinality;
an empty, short, oversized, malformed or failed page rejects the entire
request and never yields a partial `DataBatch`. Older pages precede newer
pages in the complete sequence while source order inside each page is
preserved. The complete sequence then passes BR-022 validation for duplicate
or non-increasing times and all structural fields. Provenance, observation
time and the shared record/batch identity are created only after all pages are
complete and valid. Blocking, Smart, Direct and async normalized Providers
share these semantics; downstream callers must not implement Provider-specific
TDX pagination.

## BR-032 Security lifecycle atomic evidence
Listing dates and corporate actions may authorize historical price-continuity
exceptions only through provider-neutral records with exact instrument
identity, validated source dates, record evidence and atomic batch provenance.
TDX finance `ipo_date` and raw XDXR DTOs are not consumer contracts. A listing
date must be a calendar-valid non-future `YYYYMMDD` from the matching finance
packet. Corporate-action records must use normalized status and terms; only an
implemented action with an exact effective date can explain a discontinuity.
Unknown categories, proposed/cancelled actions, invalid values, identity or
batch conflicts, duplicates, unordered/partial packets and transport failure
fail explicitly. Source-proven empty is distinct from unavailable. Local
observation time, effective date, security-code prefixes and downstream
mutable caches must not be presented as provider source evidence.
Every normalized response exposes an explicit `admission_as_of` calendar date;
coverage and effective dates later than that boundary fail, and Router binds
the same date as policy rather than trusting a Provider-selected future date.

## BR-033 Strict source-time freshness
Realtime freshness is measured only from a provider-supplied, parseable
`source_at`; local `observed_at`, fetch completion time and cache insertion time
must never substitute for it. A strict quote route validates batch and record
timestamps, rejects missing, malformed, future, inconsistent or mixed times and
uses the oldest record time for admission. During continuous trading a configured
five-second policy accepts age exactly five seconds and rejects any greater age.
Pre-open, lunch break, post-close, replay and historical use an explicitly
different policy. A source without trustworthy provider time, including current
TDX quote packets, remains eligible only for routes that do not require the strict
freshness contract.

## BR-034 Full-market ranking and breadth evidence
Every instrument ranking row retains normalized instrument code and the
source-supplied name together, an explicit metric and unit, source session/date,
continuous unique rank and atomic evidence. Full-market claims additionally
prove the requested Shanghai, Shenzhen and Beijing universe coverage, pagination,
unique identity, ordering and a bounded source-time skew before applying a caller
limit. A composed multi-request snapshot reports its coverage and maximum skew
and must not be labelled an atomic provider ranking. Market breadth uses a
separate typed snapshot: valid count equals up plus down plus flat, limit-up/down
sets are subsets of valid instruments, and every derived value retains its input
evidence. Missing names, units, source times or coverage fail instead of becoming
successful partial rankings.
For the composed breadth snapshot, `maximum_source_skew_millis` is the skew of
the dynamic quote records. Universe and limit-pool sources publish date-level
coverage only; their exact evidence is retained but must not be promoted from a
date or local fetch time into a fabricated intraday source instant.

Provider Top-N ranking is a separate, explicitly narrower contract. It may
accept only one provider-ordered response page within the source's proved page
cap, and it must retain the provider-declared total, exact inspected row count,
continuous locally assigned source-response ordinal, selected-row metric
completeness, every record's `latest_trading_date`, and one exact batch
identity. It must not claim complete-universe coverage, synthesize a common
source time, feed market breadth, or be routed through the complete-universe
ranking type. Post-close admission additionally binds every selected
latest-trading date to the exact requested China trading date. A request date
later than the current Asia/Shanghai date fails before transport. Acquisition
on the requested date requires both start and post-response observation at or
after 15:35. A later calendar-date capture is admitted at any time only when
every selected `f297` still equals the exact requested trading date; this
proves the provider's latest settled session, not arbitrary historical
retrieval. Capture before the requested date and a response crossing its
capture-calendar midnight are rejected. Per-security quote/update time and
latest-trading date must not be promoted to the ranking metric's source time;
Top-N batch evidence therefore has no `source_at` and is not eligible for
generic realtime freshness. Each metric has an independent Top-N capability
and remains unsupported until its own bounded live probe passes. Existing
full-market ranking capabilities remain false. A concrete admitted route must
construct its production provider client inside the composition layer and
must not expose client/transport injection; the provider-neutral Core trait
and Router primitives alone are not an admission witness, and
downstream-local wrappers must not impersonate a provider identity or
capability. The deterministic Top-N batch identity must bind the metric,
trading date, exact limit, admitted filter identity, post-response observation
time and a SHA-256 digest of the canonical response. Canonicalization sorts
object keys, length-prefixes scalar and container boundaries, and preserves
array/source order. Different metrics or normalized response contents must not
share an identity; JSON whitespace or object-key order alone must not change
it. Random values and locally fabricated `source_at` values are prohibited.

The gRPC `MarketRankings` operation is a second narrow contract over one bounded
Eastmoney HTTPS response. It requires every selected row to contain an exact
security identity, non-empty source name, metric, unit, continuous source rank
and positive provider timestamp. It retains the provider-reported universe size
and actual first-page row count, but does not claim complete-universe pagination,
cutoff-tie completeness or a common source instant.

## BR-035 Licensed and authenticated data boundaries
The complete Level-2 opening-auction contract requires provider-backed matched price,
previous close, change, matched quantity and amount, unmatched bid and ask
quantities, volume ratio, exact instrument identity and provider source time.
Ordinary quotes, trades and public HTML must not be used to infer unmatched
queues or fabricate source time. The narrower gRPC `Auctions` observation may
advertise only one-response source-returned matched quantity in shares and
matched amount in CNY for one exact equity and source date. Level-2-only fields
remain explicit `null`; this does not promote the complete Core auction
capability. Broker cash, positions, orders and executions belong to a
separate authenticated account gateway. Browser cookies and logged-in page
scraping are not a production account API, and this workspace must not add a
downstream account path dependency.
The complete Level-2 auction conformance policy binds an explicit provider source name,
China trading date and `09:15:00..=09:25:00 +08:00` opening-auction window;
fresh continuous-session, closing-auction or wrong-date data fails even when
all numeric fields are present.

## BR-001 Quote request cardinality
Strict quote requests accept 1 through 60 instruments. `quotes_chunked` is the only API that may split a larger request.
## BR-002 Strict pagination
Strict paginated operations are atomic.
## BR-003 Pool and queue policy
Blocking defaults to five connections; async defaults to four.
## BR-004 Smart server policy
Smart selection orders eligible servers by observed health and latency.
## BR-005 Adaptive rate limits
The compatible schedule is 15/30/60 requests per second with a 200 request-per-second ceiling.
## BR-006 Cache policy
Caching is disabled by default and never relabels stale values as fresh.
## BR-007 Security metadata evidence
Exchange is represented explicitly, including Beijing. Board derivation is
reported as derived evidence; listing dates and price-limit rule versions stay
unavailable unless the provider supplies them. Beijing is never mapped to an
unverified Shanghai/Shenzhen provider market number.
## BR-008 Financial archive integrity
Financial manifest sizes are bounded allocation hints. HTTP framing plus ZIP
entry bounds, uncompressed size, and CRC are mandatory before parsing or cache
admission.
## BR-009 Public-provider capability admission
An optional public-web capability is advertised only after deterministic
contract tests and a bounded live probe both prove the normalized records,
source identity, source time when supplied, observation time and batch
identity. Authentication-gated or unverified families remain false and return
a typed `Authentication`, `Unsupported` or protocol error.

## BR-010 Public-provider request bounds and pacing
Every public-web request enforces its verified positive row bound before I/O.
Clones of one Provider client share the same request limiter. Where the source
contract requires pacing, request starts are serialized at no less than the
documented interval; HTTP 429 and limiter failure are explicit errors and do
not trigger an unpaced retry.

## BR-011 Public-provider duplicate identity
Within one atomic Provider batch, duplicate business identities are rejected
as protocol failures. The only admitted exception is semantic-search output:
rows with the same normalized security identity are collapsed to the
source-supplied highest score, with deterministic first-seen tie breaking.
No downstream consumer may deduplicate by display name.

## BR-021 Public-provider probe admission states
An advertised public-provider family satisfies a probe only as `admitted` or
source-evidenced `verified_empty`. Ordinary empty batches, incomplete quality,
issues, provenance mismatch, future or stale source time, duplicate identity,
unit inconsistency, and cross-field inconsistency fail explicitly.
`diagnostic_complete_unadmitted`, `skipped_missing_secret`, and `failed` never
promote or satisfy a capability.

## BR-022 TDX normalized bar atomicity
The provider-facing Magic TDX historical-bar operation returns only
provider-neutral `magic_market_core::Bar` records. Raw `SecurityBar` remains a
wire/protocol DTO and is not a second `HistoricalBars` contract. One request is
atomic: declared rows must decode completely; empty, oversized, duplicate,
non-increasing, structurally invalid or internally inconsistent sequences fail
explicitly. The adapter never sorts, deduplicates, fills or mixes fields. A
fixed adjacent-close percentage is not a provider-integrity rule: legitimate
IPO, relisting, resumption, corporate-action and market-price moves may exceed
20 percent, so the Magic TDX adapter must preserve such source rows after the
same structural and provenance checks instead of rejecting or rewriting them.
Consumers that require economic jump confirmation must enforce that policy at
their own evidence boundary; provider admission is not confirmation. TDX
source bar volume is converted from shares to Core lots by dividing by 100,
amount is preserved in CNY yuan, and every record must carry
`ProviderId::Tdx`, the exact source timestamp and the same non-empty batch
identity as batch provenance.

## BR-023 TDX normalized current-session admission
Raw TDX current-minute and current-transaction packets are diagnostic evidence
only until the normalized provider or gateway verifies an active A-share
weekday session in Asia/Shanghai. Normalized current minute and trade requests
are admitted only during `09:30:00..=11:30:00` or
`13:00:00..=15:00:00`; weekends and weekday off-session windows fail before
transport and must not relabel a cached prior-session packet as current.
System-clock failure also fails closed. Requests carrying an explicit source
date bypass this wall-clock gate and continue through the historical endpoint.
This rule does not infer exchange holidays and does not fabricate a source
date.

## BR-024 Whole-market dragon-tiger disclosure admission
One whole-market dragon-tiger request has an explicit trading date and a
positive result limit of at most 100. "Market" in this rule means A-share
equities: the Provider applies the source's explicit equity security-type
filter, and a non-equity row that violates that filter fails rather than being
guessed from its display name. The Provider reads the bounded complete A-share
source day before local admission. Business identity is provider, security,
trading date and source entry ID (`TRADE_ID` for Eastmoney). Distinct source
entry IDs, including multiple reasons for one security/date, must remain
distinct. Fully equivalent duplicate records with one business identity are
stably collapsed to the first source occurrence; conflicting records with that
identity fail the atomic batch. Entries sort by present net amount descending,
then security identity and entry ID; missing net amount sorts last. The caller
limit is applied only after deduplication and sorting. Every admitted entry
must carry exactly five buy and five sell seats filtered by that entry's source
identity. A normalized seat's business identity is entry ID, side and source
order rank 1 through 5. Repeated display labels such as `机构专用`, and equal
source amounts, remain distinct when their side/rank differs; display text and
amounts are facts, not identity fields. Missing, extra, duplicate side/rank or
cross-entry seats fail explicitly; missing numeric fields are never rendered,
coerced to zero or replaced with textual numeric placeholders.

## BR-025 Sina official instrument-news admission
One Sina instrument-news request accepts one validated Shanghai or Shenzhen
A-share equity and a positive output limit of at most 200. The Provider builds
at most five official AllNewsStock HTTPS page URLs from the exact
exchange-prefixed symbol and never follows a page-supplied URL. Every page must
repeat that exact symbol in its server-rendered `page_symbol`, contain the
requested page marker, use the verified GBK-family HTML MIME, and expose a
non-empty company-news `datelist`. Records remain in source newest-first order;
start/end filtering is an inclusive source-date filter and limit is applied
only after validation and deduplication. Canonical URL is the business
identity: fully equivalent duplicates are stably collapsed to the first source
occurrence, while title or published-time conflicts fail the atomic batch.
Equivalent duplicate comparison uses source facts only; different local
observation times across page requests do not create a source conflict. A
non-empty, identity-valid page whose records all fall outside the requested
inclusive date range returns a complete zero-record batch with page
observation time, batch identity and the newest fetched source-row time in
provenance. This is provider-proven empty, not source unavailability.
An official page-supplied `http` article URL may be normalized only by changing
the scheme to `https` after structured URL parsing proves a Sina-controlled
host; username, password and explicit port are forbidden, and host, path,
query and fragment remain unchanged. The normalized HTTPS URL is the stored
business identity. Missing, invalid or future provider time, non-Sina
canonical URLs, empty pages, wrong identity, ordering changes, redirects,
response/page bounds and an unresolved fifth-page continuation fail
explicitly. Instrument identity comes only from the validated request URL plus
exact page marker, never from title or body text. The retired `feed.mix`
pageid=155 path and the global pageid=153 feed are not fallbacks.

## BR-026 TDX board-membership atomic admission
One Magic TDX board-membership request accepts a non-empty ordered set of validated
Shanghai/Shenzhen equity instruments. Exact duplicate requests collapse to their
first occurrence; the same six-digit code paired with conflicting exchange or asset
identity fails before transport. Beijing and non-equity instruments are explicitly
unsupported because the TDX block files do not prove those identities.

The Provider reads complete, version-stable `block_fg.dat`, `block_gn.dat`, and
`block_zs.dat` snapshots as one atomic evidence batch. A missing, partial, empty, or
version-changing source file fails the whole request. Output is the stable
request-order intersection with those complete files, ordered within each instrument
as Industry, Concept, then Unknown index membership and exact canonical board code.
Equivalent source duplicates collapse; conflicting identities fail. A complete
three-file snapshot proving no membership returns a complete empty batch with the
same provenance.

Board identity is the exact source filename plus exact source block name
(`tdx:<filename>:<blockname>`); display name is the exact block name. `block_fg.dat`
maps to Industry, `block_gn.dat` maps to Concept, and `block_zs.dat` maps to Unknown
because Core has no Index category. Names are never fuzzily classified as Region,
Industry, or Concept. All records carry `ProviderId::Tdx`, the same observed time and
batch ID derived from the three source file hashes. TDX supplies no provider time for
these files, so both batch and record `source_at` remain absent; local observation
time must not masquerade as source time.

## BR-027 CNInfo whole-market announcement discovery
One CNInfo whole-market announcement request has an inclusive start/end date
range and a positive result limit of at most 300. The Provider must use the
source's native market-list operation with an empty `stock` selector. It must
not enumerate instruments or relabel a per-instrument announcement operation
as whole-market discovery.

Remote requests use a fixed page size of 30. Every fetched page must have
complete and mutually consistent `totalAnnouncement`, `totalRecordNum`,
`totalpages`, `hasMore`, row-count and page-boundary evidence. Those values
must remain stable across pages. CNInfo's `totalpages` is the source's integer
quotient `totalAnnouncement / pageSize`, not a conventional page count; the
Provider separately derives the actual request-page count with ceiling
division and validates `hasMore` against consumed rows. The Provider validates complete source pages
until it has the requested number of unique records or consumes the declared
source total. The caller limit is applied only after complete-page validation,
stable source-order deduplication and conflict checks. Equivalent duplicate
announcement IDs collapse to the first source occurrence; conflicting rows
with one announcement ID fail the atomic batch. Source newest-first order is
preserved and publication times must not increase across or within pages.

Every record requires the source-supplied announcement ID, security code,
organization ID, publication time and exact `pageColumn`. `SHMB` and `SHKCP`
map to Shanghai equity, `SZMB` and `SZCY` map to Shenzhen equity, and `BJS`
maps to Beijing equity. Unknown or contradictory source identity fails; codes
are never assigned to an exchange by numeric-prefix guessing. The exact
provider publication timestamp is both `published_at` and record
`source_at`. Canonical detail and optional PDF URLs use only validated
source-supplied identity and safe CNInfo HTTPS paths.

A source response with all total fields equal to zero, `totalpages=0`,
`hasMore=false`, and no rows is a complete verified-empty batch with
provenance. Missing metadata, inconsistent totals, an empty non-zero page,
page drift, truncated pagination, transport failure or schema drift is an
explicit error and never becomes an empty batch. Router acceptance of a
complete empty batch is opt-in and disabled by default so existing routes keep
their prior failover behavior.

## BR-028 Eastmoney limit-pool completeness
Every Eastmoney limit-pool response must retain and validate the source
`data.tc` total before admission. `tc` must be a non-negative integer and may
not be smaller than the returned `pool` row count. A batch is complete only
when `tc` equals the number of validated unique pool rows. If `tc` is larger,
the Provider returns the validated rows with an explicit incomplete-quality
issue; it must not relabel a caller-bounded page as a complete whole-market
pool.

Source-proven empty is admitted only when `tc=0`, `pool` is an empty array,
`qdate` is present and equals the requested trading date. A missing `tc`,
missing or null `pool`, duplicate source instrument identity, inconsistent
date, or contradictory total is a protocol failure. The request limit remains
a transport bound; whole-market consumers must require a complete quality
report before applying any display or selection limit.

## BR-029 TDX connection-pool lifecycle
Closing a TDX connection pool while guards are active closes and removes only
idle connections. Active guards retain their reservation until return and are
tagged with an opaque pool generation; a guard from a generation invalidated
by `close_all` is closed and removed, never reinserted. Failed connect or
handshake attempts must release their pre-I/O reservation. Pool counter
transitions are checked so guard destruction cannot panic or poison the mutex.
The observable steady-state invariant is `total == idle + active`.

## BR-012 Public financial-news access boundary
Jin10 admission is limited to unlocked public type-0 flashes and type-2
articles belonging to at least one source news channel 1/2/3; channel-5-only
promotion slots are excluded. Protected details are never requested or
decrypted. The Paper admission is limited to native articles on
finance channel `25951`; any row with a non-empty external `link` is omitted
even when both forward flags claim native content, and is never relabeled.
The Paper `published_at` preserves the exact millisecond instant carried by the
raw `pubTimeLong` evidence. Neither source may infer structured security identity
from text.

## BR-013 Full-market discovery completeness
Full-market announcement and dragon-tiger discovery must read and validate the
source-declared complete result set before applying an exchange filter or caller
limit. Page totals, page counts, stable source identities and requested dates
must agree. Every returned stock row must retain both its normalized instrument
code and the source-supplied instrument name; a missing name is a protocol or
router quality failure.

## BR-014 Board membership provenance
TDX board directories and constituents are produced only from validated
`block_fg.dat` and `block_gn.dat` records. Board identities include category and
source name, duplicate board/member pairs fail, and reverse membership never
returns an unrequested instrument. TDX block packets do not contain stock names;
consumers that display names must join a separately sourced
`SecurityMetadataProvider` result through `join_board_membership_names` and
retain both evidence records. Missing names, extra metadata identities and
incomplete metadata coverage fail.

## BR-015 Global index and FX snapshots
Global-index and foreign-exchange requests are non-empty, bounded and
duplicate-free. Sina packets are accepted only at exact requested cardinality,
after GB18030 decoding and source-symbol validation. Missing or non-finite
values fail; FX source date/time is retained, while index source time remains
absent when the packet does not provide one.

## BR-016 Official policy admission
Official policy documents use only the credential-free State Council search
endpoint and canonical `www.gov.cn` document URLs. Only the `gongwen` and
`bumenfile` categories are admitted. Category identity, publication date,
document identity, requested range, duplicates, response bounds and one-second
pacing are mandatory.

## BR-017 Research document body integrity
Research-document retrieval must bind the requested report identity to the
exact source PDF URL. The production transport accepts only HTTPS
`pdf.dfcfw.com`, `application/pdf`, a `%PDF-` header and a body no larger than
32 MiB. HTML, redirects, identity disagreement and truncated or oversized
documents remain explicit failures.

## BR-018 Calendar source evidence
Economic releases preserve source indicator identity, country, schedule,
release time, previous/consensus/actual/revised values and importance; numeric
zero is not absence. CFFEX delivery-event diagnostic candidates are accepted
only from an official notice naming the requested contract month, all four
IF/IH/IC/IM products, their exact date and delivery-settlement-price wording.
If the notice does not independently state the settlement method, the
normalized method remains `NotProvided`; it is not inferred from a settlement
price. The notice publication date is retained as source-time evidence; the
delivery date is not substituted for publication time. If the notice does not
independently state the last trading date, that field remains absent rather
than being copied from the delivery date. Formula-only calendar inference is
prohibited. The production capability remains false and the production trait
returns `Unsupported` until BR-009 live admission succeeds. A successful
diagnostic must use the `diagnostic_probe_status` and
`diagnostic_complete_unadmitted` markers and must not emit the production
`live_probe_status=passed` marker.

## BR-019 Strict 15:35 post-close ranking
The post-close fund-flow ranking accepts only the current China trading date
at or after 15:35 Asia/Shanghai. Every row must contain a source instant whose
calendar date equals the request, code and stock name, main-net amount and
percentage, a unique source market identity, non-increasing main-net amount and
contiguous rank. Per-security source instants may differ because the public
ranking response is a locally observed post-close snapshot rather than one
provider-declared atomic source instant. One batch therefore uses the current
local `observed_at`; it sets batch `source_at` only when all record source
instants are identical and otherwise leaves it absent. Record source instants
remain unchanged. The Provider and router both require exact caller cardinality;
pre-window, wrong-date, unordered, duplicate or partial batches fail instead of
being relabelled as the requested ranking. This contract is not eligible for
BR-033 realtime freshness and does not prove full-market pagination or breadth.

## BR-020 Eastmoney rolling finance news
Eastmoney global latest news is admitted only from the exact first page at
`https://roll.eastmoney.com/finance.html`. The Provider validates the complete
`#artList` before applying a caller limit of at most 20. Every row must be in
the source `财经` category, use a calendar-valid newest-first minute timestamp,
have matching attribute/visible titles, and use a unique numeric article ID at
the canonical `/a/<id>.html` path on exactly `finance.eastmoney.com`,
`global.eastmoney.com`, `biz.eastmoney.com`, `futures.eastmoney.com`,
`bond.eastmoney.com`, or `hk.eastmoney.com`. These hosts are retained as
metadata links only and are not fetched by this operation. The public page does
not provide structured security identity, so records keep an empty instrument
list and may not be presented as instrument news.

## BR-030 Yonhap Chinese RSS metadata boundary
The Yonhap Provider may read only one of the seven official simplified-Chinese
RSS endpoints per bounded request. It maps title, exact canonical article
identity and URL, publication time, channel and provenance only; summary and
content remain absent and article pages are never fetched. The complete feed
must pass exact endpoint, XML structure, required-field, unique-ID/URL,
newest-first and 100-row bounds before caller-limit truncation. Public global
news capability is scoped per fixed channel. The Economy endpoint is the only
production-admitted channel after two bounded live and three serial load
observations on 2026-08-16. The production constructor therefore selects
Economy. Every other explicit channel, including Rolling, remains diagnostic
and its trait call fails before I/O; the Rolling feed currently exceeds the
100-row complete-source bound. No channel admission widens another channel's
endpoint or resource limits.

## BR-031 WallstreetCN RSS metadata boundary
The WallstreetCN Provider may read only
`https://dedicated.wallstreetcn.com/rss.xml`. It may expose only title,
decimal article ID, exact canonical URL, publication time, publisher,
language, topic, and provenance. RSS descriptions, article bodies, media,
article-page fetching, undocumented APIs, authenticated content, storage,
caching, search indexing, and inferred instruments are prohibited.
`global_news` may be advertised only after two consecutive bounded
production-client live probes pass; otherwise the trait remains typed
`Unsupported` and only the explicit diagnostic path may access the feed.

## BR-037 EMQuant fake-bridge test isolation
Every executable fake bridge used by the Unix EMQuant integration suite is a
checked-in mode-100755 fixture below the crate's `tests/fixtures` directory.
The test process must not create, write, chmod, rename or delete a pathname
that it later executes. Default, timeout and malformed-response behavior use
separate immutable fixtures. Concurrent public clients may execute one fixture
only after the test verifies it is a regular executable file. Production
EMQuant discovery, command execution, timeout and normalization semantics
remain unchanged.

## BR-039 Official economic observation integrity
Provider-native namespace, code, region, frequency, period, unit, scale and
revision facts remain source-scoped. Missing is never zero, local fetch time is
never a release time, and any failed page or series invalidates the atomic
request.

## BR-040 Official rate and fixing identity
Benchmark tenor, percent unit, base/quote orientation, quotation base and
fixing date are mandatory source facts. DR007, R007, Shibor and LPR are not
interchangeable, and an official fixing is not a realtime quote.

## BR-041 SEC filing metadata-only access
SEC requests use official submissions hosts, a descriptive redacted
User-Agent, bounded pacing and atomic older-file composition. Normalized
records expose metadata and canonical links only and never download bodies,
attachments or XBRL facts.

## BR-042 Public financial-news metadata boundary
Xinhua Finance, Yicai and Securities Times records retain only first-party
title, ID, link, publisher, publication-time and topic metadata. Bodies,
descriptions, images, login state, cookies and inferred instruments are
prohibited.

## BR-043 TDX local-terminal observation admission

The optional local-terminal source is a separately authorized read-only path
and uses the existing `ProviderId::LocalTerminal`. It never impersonates
`ProviderId::Tdx`, changes an existing Router route, or starts from a library
constructor/default feature. Production discovery accepts exactly one running
TDX executable in the current Windows user/session. Its executable identity is
recorded as provenance; a bounded request to the fixed official loopback origin
then validates the implemented response schema. Missing, ambiguous,
architecture-incompatible, endpoint-unavailable or schema-incompatible clients
start no poller and expose no event listener. An updated executable hash is not
by itself a configuration error when the same fixed read schema still passes.

Every terminal data family is false by default. Admission independently proves
instrument identity, fields, units, source time where present, local observation
time, sequence semantics, entitlement, bounded resource behavior and reconnect
or reset rules. Local time never becomes provider source time; poll sequence
never becomes provider completeness. Rust obtains TQ data only through the
vendor-documented loopback HTTP endpoint at exact origin
`http://127.0.0.1:17709/`. The client disables proxies and redirects, accepts
no alternate host, port or path, sends only an explicit read-only method
allowlist, and enforces injected positive connect/read/write timeouts plus
request/response bounds. It neither loads nor calls vendor DLLs and never falls
back to Python. Endpoint availability alone does not admit a data family:
response schema, field identity, units, source time, latency, resource cost,
terminal-exit behavior and bounded live evidence must pass independently. The
source tree must not send account, cash, position, order, cancel or execution
methods, preserving BR-035.

For the explicit `EQUITY` monitoring watchlist, a six-digit code and exchange
label are request data, not source identity evidence. Before a new terminal
generation emits any market observation, the fixed loopback client reads the
vendor all-A-share universe with the exact read-only `get_stock_list` request
and validates a non-empty, bounded, duplicate-free canonical identity list.
Every watchlist member must be an exact member; a missing/malformed identity or
universe failure prevents polling. This validation is repeated after terminal
replacement and cannot promote a data-family admission.

## BR-044 Local anomaly evidence and continuity

Local anomaly events use `ProviderId::LocalAnalysis` and bind the exact admitted
`LocalTerminal` inputs, rule identity/version, time basis, instrument, stream
generation and checked sequence used to derive them. Reconnect, terminal exit,
rollback, overrun, date/session reset, rule revision or unproved ordering resets
affected windows and reports an explicit continuity state; none becomes a
successful empty observation. Replay is bounded, same-generation and
best-effort. Unavailable replay and slow-consumer loss are explicit. Replay
delivery time does not change event identity and replay must not be described as
at-least-once. Replay sizes, restart policy, watchlist bounds and throughput
defaults are selected only from shadow evidence, never from proposal values.
Until a provider source timestamp is independently proved, the anomaly envelope
uses the timestamp captured when the monitor obtained the triggering local
observation and declares `local_observation_time` as its time basis. Agent relay
time and gRPC delivery time do not replace that timestamp. `source_at` remains
absent and the local observation time cannot satisfy strict source freshness.
The framed message also carries the exact canonical instrument at top level;
Agent and gRPC filters must not infer it from a nested analysis payload or fall
back to a synthetic terminal-wide identity.

## BR-045 Versioned external gRPC boundary

The primary external protocol is versioned gRPC over HTTP/2 in a binary-only
leaf service. Core, Router and Provider crates remain transport-neutral and do
not depend on Protobuf or gRPC. Every read family has a closed operation identity
and a bounded request/response contract; callers cannot inject provider clients,
URLs, proxies, TLS policy, executable paths or arbitrary methods. Repository
admission and runtime availability are evaluated independently before Provider
I/O, and an RPC can never promote an unadmitted capability.

An external query may explicitly opt in to a repository-registered diagnostic
handler. Opt-in is false by default and cannot select URLs or arbitrary methods.
There is no default-readable exception: every repository-unadmitted operation
must fail before Provider I/O unless the request explicitly opts in. Every
diagnostic response remains `UNADMITTED`, is forced incomplete, and carries the
repository blocker. A missing diagnostic handler still fails before Provider
I/O. Transport success, returned records, and optional-field presence never
promote repository admission or runtime availability.

Blocking Providers execute behind an explicitly bounded blocking-worker gate,
not on a gRPC/Tokio worker. Every successful response preserves provider, batch,
completeness, source evidence, units, source time when supplied and local
observation time. Failures retain a typed safe reason and never become a
successful empty response. Non-loopback bind requires explicit remote enablement,
TLS and authentication; secrets are absent from payloads, errors and logs.

TDX observations are read only on the TDX host from the fixed official loopback
origin. A same-user Windows agent connects outward to the gRPC service and never
exposes port 17709. Price/amount/volume anomaly delivery is an ordered,
generation-bound server stream with bounded best-effort replay. Gap, rollback,
slow consumer, reconnect and unavailable replay are explicit; transport success
does not change the LocalTerminal or LocalAnalysis admission rows. The schema has
no account, cash, position, order, cancel or execution service.

## BR-046 Eastmoney Miaoxiang authenticated diagnostic boundary

The Eastmoney Miaoxiang Skills API is a separately authenticated, natural-language
query source. It is not a retry alias for the public `push2` family and its API key
is read only from the server process environment, redacted from `Debug`, errors,
logs, evidence and payloads. Calls use only the exact registered HTTPS origin and
path, a bounded JSON body, disabled redirects, bounded response bytes, positive
timeouts and shared one-request-per-second pacing.

Only exact, repository-owned query templates may be sent. External callers cannot
provide natural-language text, URLs, headers or indicator identifiers. A successful
HTTP/API response does not promote admission. Response identity, protocol type,
security or universe identity, source date, indicator labels, raw scalar
cardinality and observed unit metadata must match the requested diagnostic family.

Opening-auction production observations use one fixed query and one result table
to expose source-returned volume in shares and amount in CNY. Matched price,
previous close, unmatched queues, volume ratio and provider time remain null
unless independently proved. Market-breadth production observations use one
fixed response containing listed total, up/down/flat and limit-up/down counts;
`valid = up + down + flat` and coverage are checked. One response proves
acquisition atomicity, while absent provider source instants keep source-time
skew `null` rather than fabricating zero.
These narrow records do not satisfy the complete Level-2 auction contract. Empty tables,
extra dates, identity mismatch, unit mismatch, malformed decimals or missing
required diagnostic fields fail explicitly rather than becoming a successful
empty or zero-filled record.

When the server process has a valid Key, the exact admitted auction and breadth
templates are default-readable production operations. `MoneyFlows` and
`FundFlowSeries` retain their separate public-provider production routes and the
Miaoxiang variants remain diagnostic. Without the Key, auction and breadth stay
repository-admitted but runtime-unavailable and fail before Provider I/O.

## BR-047 TDX dynamic watchlist control

An authenticated external watchlist update replaces the complete global TDX
local-monitor watchlist; it is never an implicit append, subscriber-owned union,
or delivery-filter side effect. Entries are non-empty, ordered, duplicate-free
canonical `EQUITY:SH|SZ|BJ:NNNNNN` identities and must not exceed the positive
maximum advertised by the active Windows Agent. Invalid or oversized input and
the absence of an active Agent fail before configuration state changes.

Every changed list receives a checked monotonic revision and is sent only as a
typed command over the existing authenticated Agent stream. The Agent validates
it again, replaces only the fixed monitor's `--watchlist` argument, restarts that
monitor, and reconnects with a new terminal generation binding the applied
revision and ordered list. Replay and anomaly windows never span a watchlist
replacement. Desired and applied state remain distinct and externally visible;
an accepted/restarting response does not claim loopback health or source
availability. An identical list is idempotent. This control cannot change URLs,
methods, paths, thresholds, admission, or the BR-035 account/trading exclusion.

## BR-048 TDX production observation families

The exact official TQ-Local `Now`, `Volume` and `Amount` fields are independently
admitted only as observation-time price in CNY/share, cumulative volume in lots,
and cumulative amount in CNY. Admission requires the validated A-share universe,
fixed loopback origin and method allowlist, exact decimal/unit conversion,
bounded serial live evidence, process-generation resets, schema health and
bounded output behavior defined by BR-043.

Each raw field's admission marker is frame-local and may be true only when that
same frame carries its validated value. A missing or source-unavailable price or
volume remains absent and unadmitted without demoting an independent valid
family. A contradictory local frame is rejected before external forwarding and
forces a new monitor generation; no window or continuity state crosses that
restart.

The fast `get_pricevol` path sends one non-empty, duplicate-free, bounded
watchlist in one single-flight request. The response must contain exactly the
requested identity set; missing, additional, duplicated or malformed rows reject
the complete batch. Rows are emitted in request order with checked consecutive
bridge sequences and one shared local observation time. This batching changes
neither field admission nor source-time/completeness claims. The slower
single-symbol `get_market_snapshot` amount path remains independently paced.

The source provides neither a source timestamp nor source-record-count semantics.
Both remain absent; local `observed_at` is never copied into `source_at`, and a
poll sequence is never called source completeness. The exact snapshot
`LastClose`, `Open`, `Max` and `Min` values are independently admitted as
observation-time CNY/share fields after bounded schema, unit and serial-live
validation; none is relabelled as the current-price family. Strict source
freshness and source-record count remain unavailable.

Price, cumulative-amount and cumulative-volume anomaly triggers are admitted
only when the monitor receives an explicit positive, versioned rule
configuration. The repository supplies no threshold, window, hysteresis or
cooldown default. Triggered and rearmed messages must carry a deserializable Core
`AnomalyEvent` binding the exact rule identity, endpoint evidence, input digest,
instrument and stream cursor. Warm-up, cooling and reset updates remain
unadmitted status messages. Calendar-date, session, midday, rollback, gap and
terminal-generation transitions reset affected state; unknown source continuity
remains explicit. The Agent and gRPC server independently deserialize and verify
every admitted event and reject marker-only promotion.

## BR-049 Transport and normalized-contract hardening

TDX compressed responses must be rejected before unbounded allocation, must not
exceed the repository ceiling, and, when the wire header declares an
uncompressed size, must decode to that exact size. Async connect, read, write and
response waits are bounded; the connection-pool mutex is never held while
waiting for a response. A transport failure may rebuild the configured pool,
but it may not reorder historical bars or bypass board-code preflight checks.

The local-terminal Agent reconnects only transient gRPC and command-stream
failures. Malformed or contradictory monitor frames and monitor-output closure
restart the monitor with a new generation after the configured delay. Permanent
authentication/configuration failures, server stop instructions and sequence
contradictions remain explicit terminal failures rather than retry loops.

TDX financial archives use the bounded report protocol and never fall back to
unauthenticated HTTP.

Normalized option records are constructor-sealed. Routable records expose their
actual source and observation evidence plus any record status. Router selection
rejects `Stale`, `Conflicted` and `Unsupported` records, and an explicit strict
policy may additionally require `Available`. Provenance always contains a
non-empty batch ID; malformed serialized evidence is rejected before routing.
An eight-digit calendar date is never accepted as epoch seconds.

An active TDX Agent sends bounded idle heartbeats. The server expires a stream
that sends neither an event nor a heartbeat within its configured deadline and
removes the command channel before reporting it connected. Eastmoney public and
authenticated diagnostic calls share one production pacing lane. None of these
hardening changes promotes a Provider, data family or diagnostic admission.

## BR-050 Derived market-data product boundary

`IndexQuotes`, `IntradayShape`, `T0Evidence`, `OutcomeDailyBars`, and
`UpperLimitPoolReview` are versioned external data products, not aliases that
permit an arbitrary existing Provider response to be relabelled. Their v1 JSON
contracts are fixed by `docs/integrations/grpc-derived-products.md`. Every
request is explicitly bounded and every output retains the exact normalized
Provider input evidence. Nullable source fields remain null; zero, local fetch
time and another family's evidence must not fill a missing value.

`IndexQuotes` accepts only exact index identities and requires source-backed
freshness. `IntradayShape` consumes one complete, ordered minute series and uses
only its documented deterministic arithmetic. `T0Evidence` and
`OutcomeDailyBars` are TDX-only atomic compositions with no routing, fallback or
mixed-Provider success. `UpperLimitPoolReview` requires all four exact pool
families for one trading date and may expose facts and counts but no trading
recommendation or strategy score.

The five RPCs are append-only. A product remains false and returns typed
`UNIMPLEMENTED` before Provider I/O until its complete composition,
deterministic contract suite and live admission are registered. `IndexQuotes`,
`IntradayShape`, `T0Evidence`, `OutcomeDailyBars`, and `UpperLimitPoolReview` meet that gate
through bounded deterministic compositions and two live plus three serial
observations. `T0Evidence` binds one Quote, one five-level book, the requested
daily bars and requested five-minute bars from TDX for every exact instrument.
Its response `observed_at` is the current local Asia/Shanghai observation time;
the four original evidence objects remain unchanged, and the response
`source_at` stays absent unless all four source instants are present and equal.
It is an evidence bundle, not a BR-033 realtime freshness claim. Adding an
ordinary empty record, fixture result, partial bundle or client-selected
`allow_unadmitted` path does not satisfy production admission.

## BR-051 CFFEX fixed schedule and plaintext notice diagnostic boundary

The production `FuturesDeliveryCalendar` is a versioned checked-in 2026 schedule
for IF/IH/IC/IM only. It performs no runtime network I/O, returns exact monthly
delivery/last-trading dates with `Cash` settlement, and rejects every non-2026
request before I/O. A new year requires a new reviewed revision and tests; the
runtime never extends dates by a calendar formula. The revision is grounded in
the official [equity-index futures contract rule](https://www.cffex.com.cn/hs300/)
and [2026 holiday closure notice](https://www.cffex.com.cn/jystz/20251217/46425.html).

The separate explicit diagnostic may read only the
public, credential-free HTTP origin `http://www.cffex.com.cn` and only the exact
bounded paths `/cn/jystz.html`, `/cn/jystz_<2..=120>.html`, and
`/cn/jystz/<YYYYMMDD>/<numeric-id>.html`. It sends GET only, no body, cookies,
authorization, proxy override, redirect, query or fragment; it applies the
existing positive timeout, 8 MiB response limit and shared minimum one-second
request-start interval.

Every successful diagnostic preserves `plaintext_http` as acquisition evidence
in its deterministic batch identity and blocker. The record's `notice_url` is
the same host/path normalized to the official HTTPS canonical reference only;
it is not evidence that HTTPS was used to fetch the document. The complete list
and detail still pass the existing title, publication date, contract-month,
four-product, delivery-date and settlement-price wording checks. Missing or
changed fields fail the atomic diagnostic. The response remains incomplete and
`UNADMITTED`; browser reachability, one successful notice or canonical HTTPS
normalization cannot promote production admission.

## BR-052 Baidu source technical-bar boundary

The admitted Baidu `TechnicalBars` family is limited to one validated A-share
equity, daily interval, at most 2,001 source rows, unadjusted OHLCV/amount and
optional source-supplied MA5/MA10/MA20. Missing source MA values remain absent;
they are never locally recomputed or filled with zero. Every record preserves
its source date, Baidu identity, local observation time and atomic batch ID.
Admission does not claim a complete trading calendar, adjusted-price
continuity, a corporate-action explanation or a generic Baidu
`HistoricalBars` route. Those are separate capabilities and remain false.

## BR-053 Local observation-time boundary

When a Provider does not publish one trustworthy source instant, an explicitly
registered observation-time contract may use the current local Asia/Shanghai
clock as `observed_at`. It must preserve every available record `source_at`,
leave a missing or non-common batch `source_at` absent, retain the exact input
evidence and state that the result is ineligible for BR-033 strict source-time
freshness. Local time never repairs wrong identity, missing required fields,
partial coverage, unordered data, stale source dates or licensed-data gaps.

## BR-054 gRPC news record evidence and caller cutoff

`GlobalNews` schema version 2 returns every record's complete original
`SourceEvidence`. Record provider, batch ID and observation time must agree with
the selected Provider and response batch; record `source_at` must identify the
same instant as normalized `published_at`, must not exceed `observed_at`, and
must remain in its original Provider format. Records are strictly newest first,
and response `source_at` is exactly the newest record's source string. A batch
source time is never used to construct an older record's evidence. Any missing,
mixed, conflicting, future, unordered or substituted record evidence rejects
the entire batch as non-retryable `invalid_evidence`.

`InstrumentNews` schema version 2 additionally requires the caller's exact,
unambiguous `captured_through` instant. The service may use its China calendar
date as the Provider's inclusive end date but must exclude records later than
the exact cutoff after validating complete Provider pages. It never replaces
the cutoff with the server's current date or observation time. A complete,
fully evidenced Provider batch that retains no record at or before the cutoff
is an admitted verified-empty response: records are empty, batch `source_at` is
absent, and the real upstream batch ID and observation time remain intact.
This boundary also applies to the bounded Eastmoney ranking response, Miaoxiang
auction/breadth responses and fixed CFFEX schedule: local time is observation
evidence only. It does not promote complete multi-page rankings, complete
Level-2 auctions, CFETS DR007 or IMF data.
