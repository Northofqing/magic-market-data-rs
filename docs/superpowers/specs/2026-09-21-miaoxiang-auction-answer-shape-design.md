# Miaoxiang opening-auction answer-shape design

## Gate A boundary

This design is written on the caller's 2026-09-21 instruction to investigate the
`Auctions` failure `Eastmoney protocol error: Miaoxiang returned 3 tables,
expected exactly 1` and to produce a Gate A design before any contract change.

It amends BR-046 and the `MX_OPENING_AUCTION_ADMITTED` registry row. It does not
widen any HTTP host, path, timeout, body-size, redirect, proxy or authentication
policy, and it does not change the accepted answer shape. The only transport
reused is the existing allowlisted `POST
https://mkapi2.dfcfs.com/finskillshub/api/claw/query`. Requests stay blocking and
stay in the existing one-request-per-second Miaoxiang pacing lane.

No production contract is relaxed by this design. If the caller wants a relaxed
shape instead, that is a different design and needs its own approval.

## Problem

The 2026-08-18 admission of the Miaoxiang opening-auction contract rested on
answer cardinality: five consecutive rounds returned exactly one `Auctions`
record from one table carrying both metrics with source-declared `股` and `元`
units and `DAY` granularity
([`eastmoney-miaoxiang.md`](../../integrations/eastmoney-miaoxiang.md), lines
69-91). BR-046 states the same rule: "Opening-auction production observations use
one fixed query and one result table".

On 2026-09-21 that shape was not reproducible.

- At 09:17 the fixed template failed with `Miaoxiang source date "2026-09-18"
  does not match requested 2026-09-21`. That message is downstream of
  `single_table()`, so **exactly one table was returned at that time**.
- At 11:39 and again at 11:46 the same fixed template returned `code=0` with
  **three** tables. Both bodies were 9686 bytes and structurally identical apart
  from the response timestamp field.

So the failure is not a transient provider fault and not a request defect: the
provider's answer to one fixed query changed shape **within a single session**,
which is exactly the property the admission was claiming to have proved.

The failure is also not a transport or credential fault. `code=0`, HTTP 200, and
the returned numbers are correct: `30,341,900 / 24,100 = 1259.0`, the same
auction price and volume the admitted HITHINK `CurrentAuctionObservations` path
returned for `600519.SH` on the same session.

## Observed response

Census of the three `dataTableDTOList` entries in the 2026-09-21 body:

| # | `dataTypeEnum` | metric labels | unit | `headName` |
| --- | --- | --- | --- | --- |
| 1 | `HQ` | opening-auction volume only | `股` | `2026-09-21 11:39` |
| 2 | `DATA_BROWSER` | volume and amount | `股` / `元` | `2026-09-18` |
| 3 | `HQ` | opening-auction amount only | *(empty)* | `2026-09-21 11:39` |

## Why no widening of the shape would help

The obvious relaxation is to search the response for a table that carries the two
required metrics instead of demanding that there be exactly one table. That
relaxation cannot admit this response:

- Table 1 fails `validate_exact_field_set_size(2)` (`mx.rs:112`) — one field.
- Table 3 fails the same check and additionally has an empty amount `unitName`,
  so `validate_field_set("开盘集合竞价成交额", Some("元"), "DAY")`
  (`mx.rs:114`) cannot be satisfied. The CNY unit would have to be assumed.
- Table 2 has both metrics with `股`/`元` and `DAY`, but its source date is
  `2026-09-18`, the previous trading day.
- Tables 1 and 3 carry a `headName` with a time component; `validate_single_date`
  (`mx.rs:586-598`) compares the date string exactly, so an `HQ` table cannot
  satisfy it as written even after its units are proved.

A wider relaxation that joins tables 1 and 3 inside one response would have to
supply the amount unit from somewhere other than the response. That is the
fabrication BR-046 forbids, and it would also discard the single-table atomicity
the admission rested on.

## Decision

### 1. Keep the accepted shape unchanged

One fixed query, one table, both metrics, source-declared `股` and `元` with `DAY`
granularity, exact instrument identity and exact requested source date. No
alternative table, no cross-table join, no assumed unit, and no acceptance of the
prior trading day as the requested date. The existing fail-closed behavior is
correct; what is missing is that the failure is not diagnosable.

### 2. Name the observed shape in the typed failure

`single_table()` (`mx.rs:469-477`) currently reports only a count. Two different
faults — a provider that now splits metrics across tables, and a provider that
returned duplicate tables — produce the same sentence and require a raw probe to
tell apart. The failure must instead describe what arrived: the table count, and
for each table its `dataTypeEnum`, metric labels, declared units and source date.

This reads one additional optional response field (`dataTypeEnum`) into `MxTable`.
That field is not part of any acceptance decision, so no new response shape is
admitted; it exists only so the typed failure identifies the observed shape. The
same shaping applies to the `unique_u32` and `validate_unique_field_set` label
fan-outs (`mx.rs:479-512`), which have the same blind spot.

### 3. Demote the capability from production to diagnostic

Natural-language answer cardinality is not a production data contract. That is
already the registered reasoning for `MX_DAILY_FUND_FLOW_ADMITTED=false`
(`admissions.tsv`), and the 2026-09-21 evidence shows the auction contract rests
on the same unstable property.

- `MX_OPENING_AUCTION_ADMITTED` becomes `false` (`mx.rs:19`) and its compile-time
  guard flips (`mx.rs:901`).
- The `Auctions` Miaoxiang registration moves from
  `register_handler(admitted(...))` to
  `register_diagnostic_handler(blocked(...))` with an explicit blocker
  (`grpc_production.rs:1153-1173`), matching the `FundFlowSeries` pattern at
  `grpc_production.rs:1123-1140`.
- The registry row becomes `admitted=false`, `status=blocked`,
  `live_probe_count=2`, `serial_load_count=0`, with a blocker naming unstable
  within-session cardinality. `check_admissions.py` requires exactly this
  combination: an unadmitted row must be `blocked`, must carry a blocker, and
  must agree with the code constant; an admitted row would require three serial
  loads and forbid a blocker.
- The contract table and the evidence section of
  [`eastmoney-miaoxiang.md`](../../integrations/eastmoney-miaoxiang.md) record the
  2026-09-21 observation and the demotion, and the corresponding BR-046 sentence
  is amended to say the one-table shape is the admission criterion that this
  source stopped meeting.

This creates no data gap. The admitted auction path for production is HITHINK
`CurrentAuctionObservations`, whose 2026-09-09 design is unchanged and which
returned complete records for the same instruments on 2026-09-21. Miaoxiang
`MarketBreadth` is unaffected and keeps its own admission. `Auctions` remains
reachable for diagnostics only, under the existing unadmitted-call rules.

### 4. Re-admission criteria

Re-admission is a new Gate A evidence round, not a code revert. It requires, on
one fixed query: two bounded live probes at different times of day **and** three
serial loads, each returning exactly one table with both metrics, source-declared
`股` and `元`, `DAY` granularity and the exact requested date; plus a same-session
repeat proving the one-table cardinality is stable across the session, since
cardinality is the property that failed. The two 2026-09-21 bodies stay in the
evidence record as the counter-example.

## Rejected alternatives

- **Pick the table with the most matching labels.** Admits table 2 and publishes
  the previous trading day's auction as the requested date.
- **Join tables within one response.** Requires assuming the amount unit table 3
  does not declare.
- **Relax the date to the latest available.** Same defect as the first
  alternative, stated more openly.
- **Retry on shape mismatch.** A retry is a second query, which BR-046 forbids
  joining to fill a field, and it would hide the instability instead of recording
  it. It also spends the bounded daily quota.
- **Treat the shape change as a transport fault.** The response was HTTP 200 with
  `code=0`; `TransportError` is not involved, and reclassifying it as
  `Unavailable` would advertise a retryable fault that retrying cannot fix.

## Public contract

- Operation/RPC: `Auctions` (unchanged name and request/record schemas)
- Provider: `EastmoneyMiaoxiang`
- Status: repository-unadmitted, diagnostic only, explicit blocker recorded

## Verification

- Deterministic tests: a three-table response in which the only both-metric table
  carries the wrong source date, a response in which the dated tables lack the
  amount unit, and a duplicate-table response each produce a typed failure whose
  message names the observed shape; the existing rejection tests
  (`mx.rs:1082`, `mx.rs:1098`) keep passing unchanged; the compile-time guards at
  `mx.rs:900-902` are updated with the flag.
- Registry: `tools/compliance/check_admissions.py` passes with the demoted row,
  and the `Auctions` capability reports `repository_admitted=false` with the
  blocker text over the existing capability surface.
- Live: two bounded probes of the fixed template recording the raw bodies and the
  per-table census, without logging the API key.
- Gate C: formatting, workspace tests, Clippy, the repository compliance checks
  and the documentation link check.
