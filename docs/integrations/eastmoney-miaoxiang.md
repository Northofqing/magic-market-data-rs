# Eastmoney Miaoxiang authenticated contracts

This source is separate from [`eastmoney-web.md`](eastmoney-web.md). It uses the
official Miaoxiang Skills API linked by the Eastmoney Skills page and never acts
as an implicit retry for `push2`, `push2his` or other public-web endpoints.

## Credential and endpoint

- `EASTMONEY_API_KEY` is the repository-facing environment variable.
- `MX_APIKEY` is accepted only as a compatibility alias when the primary
  variable is absent.
- The only request is `POST
  https://mkapi2.dfcfs.com/finskillshub/api/claw/query` with an `apikey` header
  and one fixed-template `toolQuery` JSON field.
- The secret is never serialized, logged or included in errors or evidence. The
  local `.env.local` file is Git-ignored; the service process must receive the
  variables at startup because the Rust binaries do not parse arbitrary dotenv
  files.
- Public Eastmoney and authenticated Miaoxiang calls share the same production
  pacing lane. Requests are blocking, bounded and executed by the gRPC service
  under its blocking-work and concurrency budgets.
- The endpoint is fixed to HTTPS 443 with no runtime override or redirect. The
  client applies positive connect/read/write timeouts, a 4 MiB response ceiling
  and a 512-byte, control-free query ceiling; clones share the minimum
  one-second request-start interval.

The official Skills listing described the financial-data Skill as covering
securities, sectors, indices, funds and bonds, including quotes, main capital
flow, valuation and financial data. Its published ordinary/pro quota was
150/500 calls per day when checked on 2026-08-15. These product statements do
not establish a stable typed data contract by themselves.

## Narrow production contracts

The production contracts deliberately expose less than the complete Core
families. Each contract uses exactly one Miaoxiang response; a second query is
never joined to fill a missing field.

| gRPC operation | Fixed request and required facts | Explicit boundary |
| --- | --- | --- |
| `Auctions` | one exact A-share identity and one source date; matched opening-auction quantity in `股` and matched amount in CNY `元`, both declared with `DAY` granularity in the same response | matched price, previous close, unmatched bid/ask quantities, volume ratio and provider source instant remain `null`; this is not the complete Level-2 `AuctionSnapshot` capability |
| `MarketBreadth` | one source date and the all-A-share universe; listed total, up, down, flat, limit-up and limit-down counts from the same response | `valid = up + down + flat`, limit counts must be subsets, and `coverage = valid / listed_total`; one-response acquisition proves atomicity, but `maximum_source_skew_millis` remains `null` because the provider supplies no field-level source instants |

Both contracts retain the exact source date, request identity, response request
ID and current local Asia/Shanghai observation time. A date is not promoted to
an intraday instant: evidence may retain the exact ISO source date, while local
`observed_at` remains distinct and is not provider source time. Missing or
duplicate tables, wrong identity/date, field metadata mismatch, inconsistent
counts, non-finite numbers or an incomplete response fail the whole request.

`Auctions` intentionally permits the Level-2-only fields to remain JSON `null`.
This is complete for the narrower gRPC observation and does not advertise the
complete Core Level-2 auction capability. When the key is absent, these
repository contracts are runtime-unavailable rather than silently routed to a
public quote or another provider.

## Evidence

The original diagnostic observations on 2026-08-15 proved useful source facts
but did not satisfy the narrow one-response contracts:

| Diagnostic | Evidence | Remaining null/unproved fields |
| --- | --- | --- |
| `600396.SH` latest price/volume/main-flow | exact security identity; 2026-08-10..14 raw daily values | intraday provider time |
| Opening auction, 2026-08-14 | 2,951,900 shares; 53,665,542 CNY; separate `股` and `元` metadata | returned through separate diagnostic responses; complete Level-2 fields and provider time absent |
| Daily five-bucket fund flow | main/super-large/large/medium/small net CNY values | exact result-count contract and serial stability |
| All-A breadth, 2026-08-14 | up 2400, down 2970, flat 170, limit-up 64, limit-down 13 | listed total, coverage and one-response atomicity were not yet proved |

On 2026-08-18, bounded contract discovery produced these single-response facts:

- `600396.SH / 2026-08-17` auction: 3,637,100 shares and 59,793,924 CNY,
  with exact `股`/`元` and `DAY` metadata in one table.
- `2026-08-17` all-A breadth: listed total 5,544; valid 5,539; up 4,335;
  down 1,064; flat 140; limit-up 110; limit-down 1; coverage is exactly
  `5539 / 5544`. All fields came from one response.

The formal production probe then completed five consecutive rounds with the same
command, followed by one additional independent successful round. The first two
consecutive rounds are the live evidence and the next three are the serial-load
evidence. Every round returned exactly one `Auctions` record and one
`MarketBreadth` record for source date `2026-08-17`; both batches were strict and
complete, and record/batch source and identity evidence agreed. Auction
Level-2-only fields remained `null` as designed.

The breadth source represented listed total as `5544.0`. The production parser
accepts a decimal spelling only when its fractional part is exactly zero and the
value is in the bounded integer domain; values such as `5544.1` still fail the
whole response. With the successful two live plus three serial observations,
`MX_OPENING_AUCTION_ADMITTED=true` and `MX_MARKET_BREADTH_ADMITTED=true`. The
additional successful round is retained as supporting evidence but is not needed
to inflate the registry counters. See [`admissions.tsv`](admissions.tsv).

## Remaining diagnostic

`MX_DAILY_FUND_FLOW_ADMITTED` remains `false`. Its natural-language daily
five-bucket result is available only as an explicit diagnostic because result
cardinality and serial stability are not a production contract. It never
replaces the admitted public Eastmoney `FundFlowSeries`/`MoneyFlows` path.

Requests for CFFEX delivery returned no table. A requested five-minute K-line
was returned at daily granularity. Requests for ranked securities returned an
all-A aggregate instead of ranked security rows. Those paths are not registered
as Miaoxiang production capabilities.

See BR-035, BR-046, BR-053 and the
[Gate A design](../superpowers/specs/2026-08-18-final-four-capability-admission-design.md).
