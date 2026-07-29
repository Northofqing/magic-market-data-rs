# Yicai News integration

## Capability state

Global-news admission is true; the formal provider and explicit probe use the
same bounded request and parser.

## Official host and paths

Only `https://www.yicai.com/news/info/` is requested.

## Request and response ceilings

HTML is capped at 2 MiB, embedded `firstlist` JSON at 512 KiB, all source rows
at 300, returned rows at 50, with one-second pacing.

## Identity, unit, missing, and source-time semantics

Only NewsID, title, creation time, source publisher and exact relative URL are
deserialized. Outer Unicode display whitespace is removed from title and
publisher only after controls and trim-to-empty values are rejected. Source
publisher attribution is preserved and Beijing time uses an explicit `+08:00`.

## Authentication or usage-rights boundary

No detail page, login, Cookie, CAPTCHA, subscriber or push endpoint is used.
Public access does not grant redistribution rights.

## Deterministic tests

Synthetic fixtures cover balanced JSON scanning, exact assignment/cardinality,
URL identity, publisher, ordering, full-list validation and discarded fields.

## Live and load admission evidence

On 2026-07-29, two consecutive live probes each returned and verified 50
complete, fresh metadata records. A three-call serial load probe verified 150
records, one active request at most, and an actual minimum request-start gap of
1000 ms. This evidence admits only the bounded first-page metadata contract.

## Explicit unsupported operations

Body/notes/images/video/share metadata, inferred instruments, instrument news
and history traversal are unsupported.
