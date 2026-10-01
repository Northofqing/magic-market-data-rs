# First domestic official publications and disclosure candidates

## Gate A decision and scope

The user approved the preceding recommendation on 2026-10-01: deepen company
disclosures first and add domestic official originals before overseas/paid feeds.
This implementation preserves that decision as an incremental Rust-library slice.
Routine details below do not change trading behavior or external credentials.

1. Extend the existing bounded disclosure candidates with shareholder increases,
   equity issuance, control changes, share transfers, pledges/releases, freezes,
   repurchases, incentives and employee ownership plans. Ambiguous titles remain
   unclassified; candidates never assert quantities or completed transactions.
2. Add a provider crate for bounded official publication listings and original
   HTML documents. Its closed source catalog covers Nbs, Pbc, Ndrc, Mof, Miit,
   Mofcom, Gacc, Nea and Csrc. Each source retains independent admission status.
   Production access requires source-specific verified evidence; explicit probe
   access may diagnose unadmitted templates. No hidden cross-source fallback.
3. Correct the existing Cninfo coverage documentation to the current 300-record
   contract; do not widen its request or claim exhaustive daily coverage.

## Publication contract and transport

Official publications are not projected into `GlobalNews` schema v2 when the
source only proves a calendar date. A dedicated record retains publisher,
channel through its fixed source profile, canonical URL, original title,
published date, exact publication label and precision, extracted text and
evidence (observation time and response SHA-256).
Date-only publication labels remain dates, never midnight instants. This slice
does not add a gRPC method, inferred policy applicability, economic values,
ownership quantities, background jobs, storage or an event subscription.

Only fixed HTTPS hosts/paths in the source catalog may be fetched. Shared
`magic-market-transport` provides no redirects, strict media/status validation,
identity content encoding, a 2 MiB body limit and 15-second default timeout.
Clones share a serialized gate with at least one second between request starts.
Every original URL is validated before I/O; cross-host links are not followed.
No provider-local HTTP/TLS dependency is introduced. `tl` and `html-escape`
provide non-network HTML parsing and entity decoding; their locked dependency
licenses are checked. An initial `scraper` experiment was removed because its
CSS dependencies failed the existing MPL-2.0 license policy. The policy remains
unchanged. HTTP registry updates precede runtime use of the new crate.

Listings inspect only a fixed current page (static HTML or the website's exact
first-page JSON request), at most 100 source items and
at most 20 returned references. The complete selected source container is
validated before applying limit. Missing/ambiguous metadata, empty source
containers, duplicate identities, unexpected article URLs, conflicting dates or
unsupported templates are typed errors. A bounded listing is not a complete
day, history or policy inventory. Cross-host entries may block a source profile
rather than be silently dropped. Detailed profile evidence will record that
boundary and any unavailable site explicitly.

## Implementation plan and acceptance

1. Extend and verify conservative disclosure classification and CLI kinds.
2. Capture current source templates and document exact source profiles.
3. Implement shared-transport listing/article probes and deterministic fixtures.
4. Run bounded live reads and serial checks. Promote only profiles satisfying
   BR-009 evidence; retain failed/unverified profiles as unadmitted with blockers.
5. Run formatting, workspace tests, Clippy, docs/link checks, compliance and
   dependency checks. Record failures and scope without overstating readiness.

Meaningful regression cases include buyback versus shareholder increase,
repurchased-share reduction versus shareholder reduction, issuance approval
versus actual issuance, unchanged control versus control change, date-only
publications, same-host malicious paths, cross-host links, duplicate listing
items, truncated layouts, future/conflicting dates and failing source reads.

## Deferred contracts

PDF event extraction, shareholder snapshots/control graphs, history cursors,
unified external news search, overseas feeds and paid credentials require their
own evidence and versioned design. The downstream service remains responsible
for archival storage, index/cluster state and derived interpretation.

## Gate C dependency remediation

The required dependency audit reported the existing shared-transport pin
`rustls =0.23.42` under RUSTSEC-2026-0285. Apply the upstream fixed patch
`=0.23.45` within the existing shared transport. Preserve its existing ring,
std and tls12 features, request/response policies and all provider HTTP registry
modes. This is a required verification repair, not a TLS-policy exception.
Re-run shared-transport/workspace tests and the formal publication probes using
the repaired build. Upstream evidence: [RustSec advisory](https://rustsec.org/advisories/RUSTSEC-2026-0285)
and [rustls maintainer advisory](https://github.com/rustls/rustls/security/advisories/GHSA-2mjx-qc3c-rqvc).
