# Business rules
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
non-increasing, invalid, inconsistent or unconfirmed greater-than-20-percent
jump sequences fail explicitly. The adapter never sorts, deduplicates, fills
or mixes fields. TDX source bar volume is converted from shares to Core lots
by dividing by 100, amount is preserved in CNY yuan, and every record must
carry `ProviderId::Tdx`, the exact source timestamp and the same non-empty
batch identity as batch provenance.

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
finance channel `25951`; externally forwarded rows are omitted rather than
relabeled. Neither source may infer structured security identity from text.

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
zero is not absence. CFFEX delivery events are admitted only from an official
notice naming the requested contract month, all four IF/IH/IC/IM products,
their exact date and cash-settlement wording. Formula-only calendar inference
is prohibited.

## BR-019 Strict 15:35 post-close ranking
The post-close fund-flow ranking accepts only the current China trading date
at or after 15:35 Asia/Shanghai. Every row must share the exact source
timestamp/date, contain code and stock name, preserve main-net amount and
percentage, use unique source market identities, be strictly non-increasing by
main-net amount and have contiguous ranks. The Provider and router both require
exact caller cardinality; stale, pre-window, mixed-snapshot or partial batches
fail instead of being relabeled as the requested ranking.

## BR-020 Eastmoney rolling finance news
Eastmoney global latest news is admitted only from the exact first page at
`https://roll.eastmoney.com/finance.html`. The Provider validates the complete
`#artList` before applying a caller limit of at most 20. Every row must be in
the source `财经` category, use a calendar-valid newest-first minute timestamp,
have matching attribute/visible titles, and use a unique numeric article ID at
the canonical `/a/<id>.html` path on exactly `finance.eastmoney.com` or
`global.eastmoney.com`. The public page does
not provide structured security identity, so records keep an empty instrument
list and may not be presented as instrument news.
