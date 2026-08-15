# Eastmoney Miaoxiang authenticated diagnostics design

Status: Gate A approved for diagnostic-only implementation on 2026-08-15.

## Objective

Add a separate authenticated Eastmoney Miaoxiang source for data that the legacy
public-web endpoints cannot deliver reliably. The initial slice exposes only
bounded, opt-in gRPC diagnostics. It does not change any repository admission flag
or the existing `EastmoneyClient` public-web behavior.

## Registered transport

- Exact endpoint: `POST https://mkapi2.dfcfs.com/finskillshub/api/claw/query`
- Authentication: `apikey` header from `EASTMONEY_API_KEY`; `MX_APIKEY` is a
  compatibility alias. The secret never enters `Debug`, errors, logs, evidence or
  response payloads.
- Request: UTF-8 `application/json`, one repository-owned `toolQuery`, maximum 512
  UTF-8 bytes. Callers cannot supply query text, host, path or headers.
- Response: HTTP 200 and `application/json`, maximum 4 MiB, no redirects, positive
  connect/read/write timeout, one request per second shared pacing.
- External access: when a valid Key is present at server startup, the four fixed
  diagnostic handlers are the default selection and require no per-request
  `preferred_provider` or `allow_unadmitted`; responses remain incomplete and
  `UNADMITTED`.
- The existing Eastmoney legacy `ureq` transport is reused; its allowlist is widened
  only by the exact `mkapi2.dfcfs.com` host registered here.

## Initial diagnostics

1. Opening auction: exact A-share identity and trading date; two single-indicator
   calls prove volume=`股` and amount=`元`. The record leaves matched price,
   previous close, unmatched bid/ask, volume ratio and source time null.
2. Market breadth: exact `全部A股` aggregate and one date; up/down/flat and
   limit-up/limit-down are preserved. Listed total, coverage and source skew remain
   null because the API did not prove them.
3. Daily money-flow series: exact A-share identity, source dates and five net-flow
   fields in CNY. It remains diagnostic because the natural-language result count
   is not an exact pagination contract; source over-return is bounded and reported.

`FuturesDelivery`, intraday `TechnicalBars`, complete `PostCloseFlows` and complete
`MarketRankings` are not enabled by this design. Live probes returned no futures
delivery table, ignored the requested five-minute granularity, or returned a market
aggregate rather than ranked security rows.

## Failure and provenance

Outer and inner status/code, `SEARCH_DATA`, non-empty request identifiers, exact
entity identity, exact source date, exact indicator labels, scalar cardinality and
units are validated before record creation. Missing/ambiguous tables and any
contradiction fail with typed provider errors. Local observation time is retained
only as observation evidence; the source date is date-level evidence and is never
fabricated into an intraday timestamp.

## Verification evidence

On 2026-08-15 the supplied local key returned HTTP 200/status 0 for:

- `600396.SH` opening-auction volume `2,951,900` shares and amount `53,665,542` CNY
  for 2026-08-14, with independent `股` and `元` metadata;
- daily five-bucket fund-flow fields for 2026-08-11 through 2026-08-14;
- all-A-share breadth for 2026-08-14: up 2400, down 2970, flat 170, limit-up 64,
  limit-down 13.

These are live diagnostic evidence, not production admission evidence.
