# Eastmoney Miaoxiang authenticated diagnostics

This source is separate from [`eastmoney-web.md`](eastmoney-web.md). It uses the
official Miaoxiang Skills API linked by the Eastmoney Skills page and never acts as
an implicit retry for `push2`, `push2his` or other public-web endpoints.

## Credential and endpoint

- `EASTMONEY_API_KEY` is the repository-facing environment variable.
- `MX_APIKEY` is accepted only as a compatibility alias when the primary variable
  is absent.
- The only request is `POST
  https://mkapi2.dfcfs.com/finskillshub/api/claw/query` with an `apikey` header and
  one fixed-template `toolQuery` JSON field.
- The secret is never serialized, logged or included in errors/evidence. The local
  `.env.local` file is Git-ignored; the service process must receive variables at
  startup because the Rust binaries do not parse arbitrary dotenv files.

The official Skills listing described the financial-data Skill as covering
securities, sectors, indices, funds and bonds, including quotes, main capital flow,
valuation and financial data. Its published ordinary/pro quota was 150/500 calls
per day when checked on 2026-08-15. These product statements do not establish a
stable typed data contract by themselves.

## Bounded live evidence — 2026-08-15

All calls returned HTTP 200 with outer and inner status/code zero and
`protocolType=SEARCH_DATA`. The key and request headers were redacted.

| Diagnostic | Evidence | Remaining null/unproved fields |
|---|---|---|
| `600396.SH` latest price/volume/main-flow | exact security identity; 2026-08-10..14 raw daily values | intraday provider time |
| Opening auction, 2026-08-14 | 2,951,900 shares; 53,665,542 CNY; separate `股` and `元` metadata | matched price, previous close, unmatched bid/ask, volume ratio, provider time |
| Daily five-bucket fund flow | main/super-large/large/medium/small net CNY values | exact result-count contract and serial stability |
| All-A breadth, 2026-08-14 | up 2400, down 2970, flat 170, limit-up 64, limit-down 13 | listed total, coverage, source skew |

The rebuilt local gRPC service then completed all four registered
`EastmoneyMiaoxiang` diagnostics over mTLS: `MoneyFlows` returned one latest
record, `FundFlowSeries` returned three ordered dates, `Auctions` returned one
partial record, and `MarketBreadth` returned one partial record. Every response
was `ADMISSION_STATE_UNADMITTED`, incomplete, and carried the exact blocker; the
missing auction and breadth fields were serialized as `null` rather than zero.

Requests for CFFEX delivery returned no table. A requested five-minute K-line was
returned at daily granularity. Requests for ranked securities returned an all-A
aggregate instead of ranked security rows. Those paths remain unavailable.

## Admission

The three repository constants remain `false`. When a valid Key is present at
server startup, the four fixed gRPC diagnostics are available only with
`preferred_provider=EastmoneyMiaoxiang` and `allow_unadmitted=true`. Responses
are still forced incomplete and repository admission cannot be promoted by HTTP
success or field presence. See BR-046 and the
[Gate A design](../superpowers/specs/2026-08-15-eastmoney-miaoxiang-diagnostics-design.md).
