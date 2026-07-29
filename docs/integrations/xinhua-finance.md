# Xinhua Finance integration

## Capability state

Global-news admission is true.

## Official host and paths

Only `https://www.cnfin.com/news/index.html` is requested.

## Request and response ceilings

HTML is capped at 1 MiB, all first-page rows (maximum 13) are validated before
returning at most 13, with one-second pacing.

## Identity, unit, missing, and source-time semantics

Numeric article ID, exact canonical path/date, title, category, publisher and
Beijing publication time are retained. Instruments are empty.

## Authentication or usage-rights boundary

No article page, login, Cookie, CAPTCHA, subscriber or push endpoint is used.
Public access does not grant redistribution rights.

## Deterministic tests

Synthetic fixtures cover exact links, entity decoding, ordering, duplicates,
unsafe URLs, full-page validation and metadata-only serialization.

## Live and load admission evidence

On 2026-07-29, two consecutive bounded live probes each returned and verified
13 current rows. The three-call serial load probe returned 39 verified records,
observed one active request at a time, and measured a minimum actual
request-start gap of 1,001 ms.

## Explicit unsupported operations

Article bodies/summaries/media, inferred instruments, instrument news and
history traversal are unsupported.
