# Miaoxiang opening-auction requested-date design

## Gate A boundary

This design is written on the caller's 2026-09-22 instruction to run the
Eastmoney diagnostic route and finish the investigation it opened.

It corrects the recorded reason for `MX_OPENING_AUCTION_ADMITTED=false`. It
changes **no** admission state, no contract, no schema, no error classification
and no transport policy. No `*_ADMITTED` constant moves. It does not widen any
HTTP host, path, timeout, body-size, redirect, proxy or authentication policy,
and it reuses only the existing allowlisted `POST
https://mkapi2.dfcfs.com/finskillshub/api/claw/query`.

It amends the BR-046 sentence, the `admissions.tsv` blocker text, the
`eastmoney-miaoxiang.md` evidence and diagnostic sections, and the
`unadmitted-provider-routes.md` row. Every one of those is a statement of
*reason*; the `auctions` verdict they justify does not change.

The 2026-09-21 design
([`2026-09-21-miaoxiang-auction-answer-shape-design.md`](2026-09-21-miaoxiang-auction-answer-shape-design.md))
remains the decision that demoted the family. This design supersedes only its
**diagnosis** and its re-admission criteria.

## Problem

The 2026-09-21 design concluded that "the provider's answer to one fixed query
changed shape **within a single session**", and demoted the family because
natural-language answer cardinality "is not stable". Re-admission was made to
require "a same-session repeat proving the one-table cardinality is stable across
the session".

A 2026-09-22 evidence round shows the cardinality is **not** unstable. It is a
deterministic function of the requested date. The 2026-09-21 observation is
consistent with that rule rather than a counter-example to it, and the
re-admission criterion as written is unachievable for the reason it names.

## Observed evidence

The fixed template `查询600519.SH在<date>的开盘集合竞价成交量和开盘集合竞价成交额`,
sent to the registered endpoint on 2026-09-22 between 11:50 and 12:00 from
separate script sessions. The census reads
`data.data.searchDataResultDTO.dataTableDTOList`, the array the adapter's
`MxTable` deserializes.

### The requested date selects the answer shape

| requested date | tables | shape |
| --- | --- | --- |
| `2026-09-22` (current trading day) | 3 | `HQ[成交量(股)] @2026-09-22 11:50`, `DATA_BROWSER[成交量(股), 成交额(元)] @2026-09-21`, `HQ[成交额(no unit)] @2026-09-22 11:50` |
| `2026-09-21` | 1 | `DATA_BROWSER[成交量(股), 成交额(元)] @2026-09-21` |
| `2026-09-18` | 1 | `DATA_BROWSER[成交量(股), 成交额(元)] @2026-09-18` |
| `2026-09-17` | 1 | `DATA_BROWSER[成交量(股), 成交额(元)] @2026-09-17` |
| `2026-09-16` | 1 | `DATA_BROWSER[成交量(股), 成交额(元)] @2026-09-16` |
| `2026-09-15` | 1 | `DATA_BROWSER[成交量(股), 成交额(元)] @2026-09-15` |
| `2026-09-11` | 1 | `DATA_BROWSER[成交量(股), 成交额(元)] @2026-09-11` |

Six past trading dates, each probed independently, each returned exactly one
`DATA_BROWSER` table carrying both metrics with source-declared `股` and `元` and
a `headName` equal to the requested date. That is the admitted shape, and it is
stable across dates and across sessions.

The current trading day returns the three-table split described above. When the
same day was probed earlier, at 11:24, the tables were identical apart from the
`HQ` timestamp.

### This explains the 2026-09-21 observation

The 2026-09-21 round recorded one table at 09:17 (failing with `source date
"2026-09-18" does not match requested 2026-09-21`) and three tables at 11:39 and
11:46. Under the date rule both readings are the *current-day* behaviour at two
points in the publication cycle:

- Before the requested day is published, the only row the source holds is the
  previous trading day's `DATA_BROWSER` table, so the answer is one table whose
  date is the previous trading day.
- After publication, the source adds two `HQ` tables carrying the live
  per-metric values, so the answer is three tables.

The change was the provider publishing the day's auction, not the cardinality
drifting. The 2026-09-21 conclusion — that no single table satisfies the
contract — is still correct for that day; the attribution to within-session
instability is not.

### An independent blocker: amount spelling

Even for past dates, where the shape is right, two of the six probed dates fail
the value parser. `parse_nonnegative_integer` (`mx.rs:838`) rejects unless every
byte is an ASCII digit:

| date | 成交量 | 成交额 | accepted |
| --- | --- | --- | --- |
| `2026-09-21` | `24100` | `30341900` | yes |
| `2026-09-18` | `11332` | `14312202.68` | **no** — fractional |
| `2026-09-17` | `14000` | `17611720` | yes |
| `2026-09-16` | `7400` | `9427082.0` | **no** — zero fraction |
| `2026-09-15` | `15800` | `20239800` | yes |
| `2026-09-11` | `32020` | `41150503` | yes |

`9427082.0` is mathematically a whole number; only its spelling has a decimal.
The breadth path already tolerates exactly that case: `parse_source_count`
(`mx.rs:855`) accepts a decimal spelling whose fractional part is all zeros, and
`eastmoney-miaoxiang.md` records that behaviour as the reason breadth admits
`5544.0`. The auction path does not, so the two parsers of one source disagree
about what a whole number looks like.

This is a second, previously unrecorded blocker, independent of the date rule. It
is why a live `Auctions` probe for `2026-09-18` passes the cardinality check and
then fails with `Miaoxiang opening auction amount is not a non-negative integer`.

## Decision

### 1. Replace the recorded reason with the observed rule

The blocker text, the BR-046 sentence, the provider document and the unadmitted
route row all currently say the answer cardinality is unstable within a session
and was not reproduced on 2026-09-21. They should instead say what the evidence
supports: the source answers a **past** trading date with the single admitted
table and a **current** trading date with a three-table split, and the current
day can never satisfy the contract.

Admission state is untouched: `MX_OPENING_AUCTION_ADMITTED` stays `false`, the
registry status stays `blocked` with `live_probe_count=2` and
`serial_load_count=0`, and `check_admissions.py`'s required combination for an
unadmitted row is unchanged. The route stays diagnostic-only.

### 2. Restate the re-admission criteria

The 2026-09-21 criterion — "a same-session repeat proving the one-table
cardinality is stable across the session" — tests a property the evidence shows
was never in doubt, and cannot pass for the current trading day at any time.
Replacement criteria, on the one fixed query:

- six past trading dates in one evidence round, each returning exactly one table
  with both metrics, source-declared `股` and `元`, `DAY` granularity and the
  exact requested date;
- the amount-spelling blocker resolved first (decision 3), or each probed date
  chosen to have a whole-yuan amount, with that selection stated plainly as a
  limitation rather than presented as a general result;
- one current-trading-day probe retained as the recorded counter-example.

The two 2026-09-21 bodies and the 2026-09-22 current-day body stay in the
evidence record.

### 3. Record the amount-spelling blocker; do not fix it here

Fixing `parse_nonnegative_integer` is a code change and needs its own Gate A. It
also splits into two cases that must not be decided together:

- **Zero fraction** (`9427082.0`). This is the case breadth already admits, and
  accepting it makes the two parsers agree. It is a parsing fix.
- **Non-zero fraction** (`14312202.68`). Real auction amounts carry fen. Whether
  the contract's amount field may hold a fractional CNY value, or must be
  rounded, truncated or scaled, is a business-rule decision, not a parser
  detail. It is not decided here.

Until both are decided and implemented, `Auctions` from this source cannot be
relied on for an arbitrary past date, which is a second independent reason the
family stays diagnostic-only.

### 4. State what stays true

`EastmoneyMiaoxiang / Auctions` cannot serve the current trading day, and that is
a property of the source, not a defect to be fixed. The admitted current-auction
path is HITHINK `CurrentAuctionObservations`, unchanged and unaffected.
`MarketBreadth` keeps its own admission; this design does not touch it.

## Rejected alternatives

- **Re-admit for past dates only.** The contract makes no past-only claim, the
  amount-spelling blocker would still fail roughly a third of dates, and a
  date-scoped admission is a different contract needing its own Gate A.
- **Keep the within-session-instability reason.** Contradicted by six past dates
  returning the admitted shape in the same sessions that returned the split for
  the current day.
- **Relax the parser here to unblock past dates.** Mixes a parsing fix with an
  undecided monetary representation question, and would admit a fractional
  amount whose rounding rule no business rule states.
- **Retry the current day until one table arrives.** A retry is a second query,
  which BR-046 forbids joining to fill a field; the current day never converges
  to the required shape, so the retry only spends the bounded daily quota.

## Public contract

- Operation/RPC: `Auctions` — name, request schema and record schema unchanged.
- Provider: `EastmoneyMiaoxiang` — registration and admission state unchanged.
- Transport: no new host, path, timeout, body-size, redirect, proxy or
  authentication policy change.
- Changed: the recorded reason for `MX_OPENING_AUCTION_ADMITTED=false`, the
  BR-046 sentence, and the re-admission criteria. No behaviour changes.

## Verification

- Registry: `tools/compliance/check_admissions.py` passes with the amended
  blocker text and the unchanged `admitted=false` / `blocked` combination.
- Documentation: the link check passes, and `eastmoney-miaoxiang.md`,
  `unadmitted-provider-routes.md`, `business_rules.md` and `admissions.tsv`
  agree on the rule and on the control case.
- Live: the census is reproducible with the fixed template and the
  `dataTableDTOList` path recorded above; the API key is never logged.
- Gate C: `cargo fmt --all -- --check`, `cargo test --workspace --lib --bins
  --tests`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `tools/compliance/check.sh`. No Rust source changes, so these are
  regression-only.

## Rollback

Revert the documentation commit. The registry returns to the 2026-09-21 wording
and admission state is unaffected either way, because this design never changed
it.
