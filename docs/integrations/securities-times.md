# Securities Times integration

## Capability state

The Securities Times quick-news global admission is false. The bounded parser
remains available only through explicit diagnostic access while the live source
contract is re-audited.

## Official host and paths

Only `https://www.stcn.com/article/list.html?type=kx` is requested with the
page-declared XHR headers and no initial cursor keys.

## Request and response ceilings

JSON is capped at 2 MiB, first-page rows at 30, and request starts are one
second apart.

## Identity, unit, missing, and source-time semantics

State, cursor, ID, two exact relative URL fields, title, source attribution,
paired millisecond/second time and per-row page cursor are validated. An empty
source label is attributed to the first-party Securities Times page; nonempty
syndicated publishers are preserved. Beijing publication time is retained and
instruments remain empty.

## Authentication or usage-rights boundary

No detail page, login, Cookie, CAPTCHA, subscriber or push endpoint is used.
Public access does not grant redistribution rights.

## Deterministic tests

Synthetic fixtures cover envelopes, terminal shape, cursor consistency,
timestamps, URLs, ordering, duplicates and discarded body/share fields.

## Live and load admission evidence

On 2026-07-29, two consecutive bounded live probes each returned and verified
30 current rows. The three-call serial load probe returned 90 verified records,
observed one active request at a time, and measured a minimum actual
request-start gap of 1,001 ms.

On 2026-08-24, the live endpoint returned source attribution that failed the
non-empty/unpadded safety contract. The provider now fails closed and is no
longer production-admitted. It must not synthesize, trim, or relabel unsafe
attribution to recover admission; a fresh upstream audit and live/load evidence
are required before `GLOBAL_NEWS_ADMITTED` can become true again.

## Explicit unsupported operations

Content/share descriptions, inferred instruments, instrument news and
historical cursor traversal are unsupported.
