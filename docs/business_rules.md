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
zero is not absence. CFFEX delivery-event diagnostic candidates are accepted
only from an official notice naming the requested contract month, all four
IF/IH/IC/IM products, their exact date and delivery-settlement-price wording.
If that event notice does not independently state the settlement method, the
normalized method must remain `NotProvided`; it must not be inferred from the
existence of a settlement price. Formula-only calendar inference is
prohibited. The production capability remains false and the production trait
returns `Unsupported` until BR-009 live admission succeeds.

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
the canonical `finance.eastmoney.com/a/<id>.html` path. The public page does
not provide structured security identity, so records keep an empty instrument
list and may not be presented as instrument news.
