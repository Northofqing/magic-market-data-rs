# Bounded news and disclosure discovery design

## Decision and scope

The user approved the 2026-09-25 news-gap remediation plan. This slice restores a
deterministic `GlobalNews` default, enforces the existing Report-only iWencai
admission, and adds a Rust composition interface for searching *candidates*
inside already admitted, bounded source windows. It introduces no HTTP host,
credential, background crawl, persistence, or new gRPC operation.

The source article or announcement remains the record of evidence. A local
title/text match is a candidate label, not a verified real-world event, source
category, complete historical search, or assertion about financial quantities.

## Routing and admission

- An unpinned `GlobalNews` call prefers the explicitly configured admitted
  `WallstreetCn` handler; an explicit Provider selection remains pinned.
- iWencai `SemanticSearch` admits `Report` only. `News`, `Announcement`, and
  `General` return typed `Unsupported` before transport. The capability scope
  says Report-only; successful diagnostic News responses do not grant admission.
- Upstream HTTP article links remain invalid under the existing `HttpsUrl`
  contract. No row is silently dropped and no URL is rewritten.

## Discovery interface

The composition module accepts a `BlockingQueryGateway`, so production uses the
registered handlers and tests can use a deterministic in-memory gateway. Its
small interface has two read-only calls:

1. `search_recent_news` queries only admitted, runtime-available `GlobalNews`
   Providers with the existing version-2, limit-20 request. It matches each
   requested entity group against title, summary, and content; alternatives
   inside a group are aliases, and every group must match. It returns original
   canonical record payloads in Provider order with source identity and an
   explicit per-Provider outcome and aggregate retrieval status. It does not
   merge evidence or assert complete topic coverage. A source failure is visible,
   never converted to no hits; the example exits nonzero when no source succeeds.
2. `discover_disclosures` queries admitted Cninfo `MarketAnnouncements` for one
   explicit market day (at most 300 returned rows), or `Announcements` for one
   issuer and an explicit date range (at most 200 returned rows). Conservative
   title classification yields candidate kinds:
   annual report, half-year report, shareholder reduction, repurchased-share
   reduction, and earnings forecast. Unknown or ambiguous reduction wording
   remains unclassified. The exact source payload is retained; no share count,
   percentage, reporting metric, or effective date is inferred.

Both results state the inspected limit and do not claim historical or
full-market completeness. There is no cross-Provider `DataBatch` because one
batch provenance cannot represent multiple source batches.

## Acceptance

- Deterministic tests verify default selection despite Eastmoney registering
  first, and pinned Eastmoney behavior.
- A transport-counting iWencai test proves all unadmitted channels fail before
  network I/O while Report fixtures continue to pass.
- Discovery fixtures distinguish Rubin from unrelated chip stories, Muse Spark
  from Muse agent, annual from half-year reports, and shareholder reduction from
  repurchased-share reduction. Partial Provider failure remains explicit.
- Formatting, focused tests, Clippy, compliance, and documentation checks run
  before any release claim.

## Deferred gates

Historical Rubin/Muse coverage and an external unified query require a separately
approved, versioned contract and source-specific license/admission evidence.
iWencai News needs official confirmation of entitlement, Skill ID/version,
quota, and display/storage terms before a News live probe can promote it.
Adding official vendor or technology-media feeds requires its own Gate A design,
registered transport scope, deterministic fixture and bounded live evidence.
