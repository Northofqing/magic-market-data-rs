# MarketAnnouncements coverage version 2

## Gate A decision

This is the scoped design for the already-authorized correction of CNInfo
market-list truncation. A bounded prefix must not advertise complete coverage
of the source's declared result set. The existing native endpoint, allowlist,
TLS, body limit, timeout, page size 30, configured maximum 10 pages and caller
limit 1 through 300 remain unchanged. There is no all-category corporate-action,
exchange-wide authority, historical finality or point-in-time admission here.

Keep the existing gRPC method and version 1 request/record shapes. Correct its
`complete` value: it is true only after source exhaustion, no caller truncation
and no duplicate identity overlap. Incomplete prefixes retain validated records,
exact batch provenance and quality issues; malformed pages, conflicting duplicate
rows, changing totals, failed requests and missing page budgets still fail typed.
Equivalent duplicates can be collapsed for compatibility, but cannot prove full
pagination coverage because an overlap may displace another declared source row.

Version 2 uses the same request schema `magic.market.market_announcements.request`
and exact `start`, `end`, `limit` fields with `schema_version=2`. It returns one
`magic.market.market_announcements.coverage` version 2 record. The envelope binds
the original request bytes with SHA-256, request ID, normalized request, records,
batch provenance and explicit page/count/terminal coverage. Version 1 continues
returning individual `magic.market.announcement` version 1 records.

## Coverage evidence

The provider owns one shared pagination/normalization implementation behind
`market_announcements_with_coverage`; the existing Core trait delegates to it.
Coverage reports declared total, expected/read page counts, inspected raw rows,
unique rows, returned rows, equivalent duplicate rows, terminal `hasMore`, source
exhaustion, caller truncation and verified-empty status. Every inspected page
retains its requested page number, exact native request-body and response-body
SHA-256 hashes, byte count, row count and source metadata. The final batch identity
binds range, caller limit, counts and the ordered page evidence digest, and is
shared by every announcement record.

`complete=false` and `source_exhausted=false` for the reported 300-of-722 prefix.
Even when the final source page has been read, caller truncation or duplicate
overlap makes output coverage incomplete. Complete empty is admitted only from
the checked first-page all-zero metadata, `hasMore=false`, no rows and exact native
date-range request. This proves CNInfo's native query result, not a complete
exchange event universe, later revisions or all company-action classes.

Hashes bind the exact inspected bytes; they do not themselves provide original
bytes, source signatures, revision guarantees or point-in-time publication state.
The full envelope and original gRPC payload bytes belong in the public RPC receipt.

The native `source_total_pages` is preserved as received; this endpoint uses
the integer quotient of total rows divided by page size. It is not the same
field as `expected_request_pages`, the locally calculated ceiling needed to
inspect all declared rows. For the observed 722-row query they are 24 and 25.
Neither value alone proves a terminal page was consumed.

## Verification plan

The read-only `correlated_query_probe` example captures business RPCs without
changing routes. Plaintext remains numeric-loopback-only; explicit mutual TLS
also permits the known VM ports 50051 and 50056, verifies the CA and server name,
and reads bounded fixed-name PEM files. A token environment variable supplies
authentication; credentials are absent from its JSON receipts. Same-channel
health before/after, the selected Provider's capability, original record bytes
and original typed-error trailer bytes are retained. Capture exit zero is not
a business acceptance verdict.

Before the fix, deterministic provider tests must fail on an unexhausted prefix,
a fully inspected page truncated by the caller and a duplicated boundary row.
After the fix, test those cases plus full pagination, exact verified empty, page
hashes, stable batch identity, malformed/short/missing pages and source-total drift.
Composition tests cover version dispatch, request hashing, one envelope record,
quality propagation and unchanged version 1 shapes. Run formatting, relevant
tests and Clippy, documentation and compliance checks. A real isolated RPC for
2026-10-01 must retain the full response, request ID and same-build health identity.
No existing production listener is replaced by this work.

## Contract and deployment identity

The version 2 envelope has `request_id`, `request_payload_sha256`, normalized
`request`, `coverage_scope=CninfoNativeDateRangeQuery`, `pit_guarantee=false`,
`exchange_event_universe_complete=false`, and `result`. The latter holds the
same-run `batch` (records, provenance and quality) and `coverage`. The outer
QueryResponse's completeness and evidence mirror that batch. Verified empty
therefore still has one version 2 envelope record with zero nested announcements;
version 1 keeps its zero-record empty response.

The protobuf descriptor is unchanged by this canonical-payload schema version.
The implementation source and binary identity must still change and be reported
exactly. A new public bundle must identify that implementation and document both
payload versions; it must not be presented as the existing production build
`67c832e43f36f188e4d769f409691c0b1d9a2ea2` or mixed into Mac's sealed candidate.
