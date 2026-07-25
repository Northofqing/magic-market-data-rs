# R-04 Market Dragon-Tiger Discovery Design

**Date:** 2026-07-25
**Status:** Approved under the standing unified-data migration instruction
**Parent design:** `2026-07-23-a-stock-data-parity-design.md`

## Objective

Provide the provider-neutral, evidence-preserving input required by R-04:
bounded whole-market dragon-tiger discovery for one trading date, with the
complete buy-five and sell-five seats for every admitted entry.

The operation uses real Eastmoney public datacenter responses. It never
fabricates a missing amount, seat, reason, timestamp, or instrument identity.

## Interface and seam

Core adds:

- `MarketDragonTigerRequest`: explicit trading date and positive result limit;
- `DragonTigerDisclosure`: one `DragonTigerEntry` and exactly five buy plus
  five sell `DragonTigerSeat` records for the same entry ID, instrument and
  date;
- `MarketDragonTigerData`: one method returning
  `DataBatch<DragonTigerDisclosure>`.

This is the external seam. Whole-market paging, Eastmoney `TRADE_ID`, stable
deduplication, sorting, per-entry seat requests, and atomic failure remain
inside the Eastmoney Adapter.

## Source protocol

Discovery uses `RPT_DAILYBILLBOARD_DETAILSNEW` with explicit `TRADE_DATE` and
source `SECURITY_TYPE_CODE=058001001` A-share equity filters. Convertible-bond
rows exposed by the broader report are outside this A-share capability and are
not inferred from names or code prefixes. Eastmoney `TRADE_ID` is the source
business identity for one instrument/date/reason entry. Seat details use
`RPT_BILLBOARD_DAILYDETAILSBUY` and
`RPT_BILLBOARD_DAILYDETAILSSELL`, filtered by the discovered
`SECURITY_CODE`, `TRADE_DATE`, and `TRADE_ID`.

Filtering seats only by security and date is forbidden because a security may
appear for several reasons on one date and the response then interleaves
several independent top-five groups.

## Admission rules

1. Discovery reads the bounded complete trading-day result before local
   deduplication, sorting, and limiting.
2. Business identity is provider + instrument + trading date + `TRADE_ID`.
3. Distinct `TRADE_ID` values remain distinct even when instrument, date,
   amounts, or display text match.
4. Byte-for-byte equivalent normalized records with the same business
   identity collapse deterministically to the first source occurrence.
5. Conflicting records with the same business identity fail the whole batch.
6. Results sort by present net amount descending, then instrument identity,
   then entry ID. Missing net amount sorts after present values.
7. The caller limit is applied only after rules 1 through 6.
8. Every admitted entry must have exactly ranks 1 through 5 on both buy and
   sell sides. Missing, extra, duplicated, or mismatched seat rows fail the
   whole operation.
9. Missing optional buy/sell/net fields remain `None`. A missing side amount
   required for that seat is an explicit protocol error.
10. Entry, seats, and batch carry Eastmoney provider identity, source trading
    date, observation time, and one shared non-empty batch ID.

## Failure modes

- unsupported/invalid security identity: typed request/protocol error;
- absent or malformed `TRADE_ID`: protocol error;
- response date different from the request: protocol error;
- result reaches the 10,000-row source bound: incomplete protocol error;
- one seat group is not exactly 5+5: incomplete protocol error;
- source pagination, HTTP, decoding, or schema failure: explicit Provider
  error; no partial `DataBatch`;
- empty trading day: explicit no-data protocol error; an ordinary empty
  `DataBatch` is not admitted or relabelled as source-verified empty.

## Router validation

The Router adapter checks the request date, result limit, stable ordering,
unique entry IDs, disclosure completeness, and evidence/batch identity before
admission. It does not reparse source fields or repair Provider output.

## Validation and rollback

Fixture tests cover multi-reason preservation, exact duplicate collapse,
conflicting duplicate rejection, stable net-amount ordering, post-dedup limit,
exact `TRADE_ID` seat filtering, 5+5 atomicity, and provenance. A bounded live
probe accepts an explicit trading date through
`MAGIC_EASTMONEY_DRAGON_TIGER_DATE`.

Rollback is the exact reversal of this slice's Core, Eastmoney, Router, probe,
documentation, and test changes. No TDX files are part of this design.
