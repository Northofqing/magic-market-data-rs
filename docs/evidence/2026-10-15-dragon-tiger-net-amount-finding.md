# Dragon-tiger net amount finding (2026-10-15)

Scope: root-cause note for the `dragon_tiger.discovery` / `dragon_tiger.market`
failures that the 2026-10-07 probe batch recorded and the 2026-10-15 re-probe
batch reproduced (sample date 2026-09-30). No production code or rule was
changed; the disposition decision belongs to a Gate B item (see below).

## Reproduction

- 2026-10-07 batch: both families failed with Core
  `invalid request: dragon-tiger net amount must equal buy amount minus sell amount`
  (`docs/evidence/2026-10-07-upstream-live-probes-230223/magic-eastmoney-rs.stderr.log`).
- 2026-10-15 re-probe with the same 2026-09-30 sample date:
  `docs/evidence/2026-10-15-upstream-live-probes-214528/magic-eastmoney-rs.20260930-run.stdout.log`
  shows the identical two failures; all other eastmoney families passed.

## Root cause: one inconsistent source row, not a parser defect

Direct bounded read of `RPT_DAILYBILLBOARD_DETAILSNEW` for 2026-09-30
(datacenter `get`, 84 rows) shows exactly one row violating the Core guard
(`crates/magic-market-core/src/signals.rs` `validate_net_amount`,
tolerance absolute 0.01 yuan / relative 0):

| SECUCODE | TRADE_ID | BUY_AMT | SELL_AMT | NET_AMT | net - (buy-sell) |
| --- | --- | --- | --- | --- | --- |
| 688137.SH | 100421554 | 11697284100 | 11697284200 | 0 | +100 yuan |

The same report for 2026-10-14 (76 rows) and the 2026-10-15 discovery response
(66 records) contain no violating row; the eastmoney re-probe on 2026-10-15
completed all families including both dragon-tiger routes. The failure is
therefore source-data conditional (a row whose NET was published as 0 while
BUY-SELL = -100), not systemic.

## Why the batch fails atomically

`DragonTigerEntry::new` validates `net = buy - sell` for any record carrying
all three amounts; one violating row rejects the whole batch (fail-closed, as
the 2026-10-07/2026-10-15 probe logs show). BR-024 does not require
`net = buy - sell`: net is optional ("missing net amount sorts last"), and
missing numeric fields must never be coerced. The Core guard is stricter than
the source and the rule.

## Open decision (Gate B), recorded for the operator

- (a) Adapter-level: when the source net contradicts buy-sell beyond tolerance,
  withhold `net_amount` (BR-024 "missing net" semantics) and record a quality
  issue instead of rejecting the batch; or
- (b) Keep the strict Core guard: the day's batch fails, and the source-quality
  problem is classified as an external blocker (no ledger row exists for these
  routes today).

Evidence artifacts: `target/probe-batch/dt_20260930.json` (raw report copy),
the two batch log files cited above, and the 2026-10-15 batch summary.
