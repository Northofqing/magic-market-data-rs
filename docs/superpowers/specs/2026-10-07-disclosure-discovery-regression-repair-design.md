# Preserve bounded disclosure candidates and mixed-title ambiguity

## Gate A scope and intent

This is a corrective slice within the user-approved bounded-discovery and
official-disclosure designs, BR-066/067, and the direct instruction to repair the
project's reported gaps and integrate all source. The pinned c72 Spec review
identified two existing defects. It is not an approval for new source admission,
history/PIT coverage, gRPC search, release-gate exceptions or runtime activation.

The pre-agreed seam remains `discover_disclosures` with its existing
`BlockingQueryGateway` adapter. No injectable provider client, transport,
listener or new query operation is introduced. Tests exercise this interface
with the in-memory adapter already authorized by the bounded-discovery design.

## Bounded source inspection

A valid admitted Cninfo response may be an incomplete, validated source prefix.
Discovery must inspect its returned records rather than discard all candidates.
Preserve the source's `QueryResult.complete` as the explicit
`SourceOutcome::Inspected.source_complete` boolean; it describes the source
batch only, never complete historical or market-event coverage. The existing
`AllSourcesInspected` status means successful inspection, not source exhaustion.
The news path retains its existing rejection of incomplete news batches.

This is an intentional Rust interface addition. Exhaustive struct-variant
patterns must include the new field or `..`; the repository example will print
it. It does not change any protobuf request, record, schema version or RPC.
Requests stay pinned to Cninfo, market-day limit 300 and issuer-range limit 200.
Provider/admission conflicts, oversize results, invalid canonical records and
typed acquisition failures remain errors, with no partial candidate page.
Complete and verified-empty batches retain their actual source flag.

## Conservative title labels

Consider all supported candidate kinds before deciding uniqueness. Report,
forecast and repurchased-share-reduction wording may not bypass the same
ambiguity check used for other kinds. Mixed applicable kinds remain unclassified.
Remove half-year report spellings only for the annual-report check, so an ordinary
half-year title does not falsely match its embedded annual spelling, while a
title explicitly naming both report kinds remains ambiguous. Repurchased-share
reduction continues to take its specific role rather than also matching generic
shareholder reduction. Plans/results retain original payloads and wording; no
quantities, effective dates, completed transactions or source categories are inferred.

## Verification and integration

Keep the full disclosure test module outside `src/`, retaining its private
module link and original cases; production coverage may not count test bodies.
Use separate red/green cycles at the existing disclosure seam: first a valid
incomplete prefix, then mixed titles. Retain original failures. Add controls for
complete/empty flags, malformed incomplete rows, identity/admission/limit guards
and error propagation. Preserve all earlier tests and historical evidence.
Resolve upstream integration conflicts by retaining source intents and final
versus initial deployment records separately; stage only explicit task files.
Run formatting, focused/library/workspace tests, strict Clippy, Rustdoc,
compliance and original documentation checks. Review the resolved source delta
and dispatch/observe the unchanged original coverage CI at the exact published
successor. The 80%/95% thresholds remain fixed; source submission is not Gate D.
