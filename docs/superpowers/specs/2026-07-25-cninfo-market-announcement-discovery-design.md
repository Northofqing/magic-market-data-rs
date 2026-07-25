# CNInfo Whole-Market Announcement Discovery Design

## Status

Gate A approved on 2026-07-25. This design implements BR-027 and does not
change downstream `stock_analysis`.

## Problem

The existing `Announcements` contract is deliberately instrument-scoped. It
performs an organization lookup and then queries one security. R-08 needs a
native whole-market discovery operation. Calling the instrument API in a loop,
or removing its request identity checks, would not prove whole-market
coverage, global ordering, or pagination completeness.

CNInfo exposes a separate public list operation on
`POST https://www.cninfo.com.cn/new/hisAnnouncement/query`. A bounded live
request with `stock=` empty, `tabName=fulltext`, a fixed date, and
`pageSize=5` returned source totals, `totalpages`, `hasMore`, and records from
the market list. Plate-specific probes independently returned Shenzhen
`pageColumn=SZCY` and Beijing `pageColumn=BJS` rows. This is sufficient to
design a native market-list provider; capability admission still requires the
bounded production-trait probe in Gate D.

## Public contract

Core adds a separate `MarketAnnouncementRequest`:

- inclusive required `start` and `end` dates;
- positive `limit`, at most 300;
- construction fails when `start > end`.

Core adds `MarketAnnouncements`, returning
`DataBatch<Announcement>`. The instrument-scoped `Announcements` trait remains
unchanged. This prevents either contract from silently broadening the other.

Router adds `MarketAnnouncementRouter` and
`market_announcement_source`. Empty-batch selection is controlled by a new
`AcceptancePolicy::with_accept_complete_empty(bool)` flag. Its default is
`false`; only a route that understands provider-proven empty semantics enables
it.

## Provider architecture

`magic-cninfo-rs/src/market_announcements.rs` owns the whole-market operation.
It reuses the existing validated HTTPS transport, endpoint allowlist, response
size cap, request limiter, URL validators and Core `Announcement` record.
It does not use organization mapping and does not call
`Announcements::announcements`.

Every request sends:

- `stock=` (empty native market selector);
- `tabName=fulltext`;
- fixed `pageSize=30`;
- monotonically increasing `pageNum`;
- `column=szse`, matching the source web protocol;
- empty `category`, `plate`, `searchkey`, `secid`, `sortName`, `sortType`;
- the exact inclusive `seDate=start~end`;
- `isHLtitle=false`.

The implementation reads complete pages until it has `limit` unique records
or consumes `totalAnnouncement`. It never fetches more than the configured ten
pages.

## Pagination and atomicity

Each page must contain:

- numeric `totalAnnouncement` and `totalRecordNum`, equal to each other;
- numeric `totalpages`, equal to CNInfo's integer quotient `total / 30`;
- boolean `hasMore`, equal to whether consumed rows remain below `total`;
- `announcements`, with the exact expected row count for that page.

CNInfo's field is not a conventional page count: the bounded live response for
2026-07-24 returned `totalAnnouncement=1108`, `pageSize=30` and
`totalpages=36`, while the complete request-page count is 37. The Provider
therefore validates the source quotient exactly and separately derives the
actual request-page count with ceiling division.

All totals remain identical across pages. A page beyond the declared total, a
short page, an empty non-zero page, changing total, wrong `hasMore`, missing
metadata, configured-page exhaustion, or source ordering change fails the
entire operation. Limit is applied only after these checks and stable
deduplication.

The source order is newest first. Publication timestamps must be
non-increasing across the complete fetched prefix. Equal announcement IDs with
identical source facts collapse to the first occurrence. Any difference in
security identity, organization ID, title, publication time, category, page
column, or attachment path is a conflict and fails.

## Identity and evidence

Each record requires:

- `announcementId`;
- six-digit `secCode`;
- `orgId`;
- `announcementTitle`;
- millisecond `announcementTime`;
- exact `pageColumn`.

Source identity maps only as follows:

| pageColumn | Core identity |
| --- | --- |
| `SHMB`, `SHKCP` | Shanghai equity |
| `SZMB`, `SZCY` | Shenzhen equity |
| `BJS` | Beijing equity |

Unknown columns fail. Numeric code prefixes are not used to infer exchange.
The code is validated as six ASCII digits, then paired with the source column.

The exact provider timestamp becomes `published_at` and record `source_at`.
All records share `ProviderId::Cninfo`, one local observation time and one
batch ID containing the requested range, pages read and declared source total.
Batch `source_at` is the newest returned provider timestamp. Canonical detail
URLs use the record's source code, source organization ID, announcement ID and
publication date. Optional PDF URLs use only safe relative CNInfo paths.

## Complete empty and failure semantics

Only this exact source state is verified empty:

- `totalAnnouncement=0`;
- `totalRecordNum=0`;
- `totalpages=0`;
- `hasMore=false`;
- no announcement rows.

It returns a strict zero-record batch with fetch time and batch identity.
There is no provider record time, so batch `source_at` is absent. Missing
fields or transport/protocol errors return typed failures. The Router may
select this batch only when `accept_complete_empty=true`; default routing still
treats an empty batch as `NoData`.

## Tests

Public-interface fixture tests cover:

1. native empty-stock request and two-page whole-market mapping across
   Shanghai, Shenzhen and Beijing;
2. source order, unique ID, source time, canonical/PDF URL and batch evidence;
3. limit after complete-page validation and stable equivalent deduplication;
4. conflicting duplicates and identity columns;
5. total, total-pages, `hasMore`, row-count and cross-page drift;
6. exact verified empty versus incomplete/invalid empty;
7. Core request bounds and serde reconstruction;
8. Router validation and opt-in complete-empty selection.

The bounded live probe uses a one-day range and a limit of three. It prints the
source identities, IDs, provider times and batch evidence, and admits only a
non-empty strict batch or the exact provider-proven empty state.

## Rollback

Revert the Core market contract, CNInfo market module, Router adapter/policy,
tests, probe, BR-027 and this design together. The existing instrument
announcement contract remains untouched and is the operational fallback only
for explicitly instrument-scoped callers, never for market discovery.
