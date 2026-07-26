# Magic TDX Normalized Bars Router Design

**Date:** 2026-07-24  
**Status:** Approved for implementation  
**Decision owner:** user direction: “Magic TDX 主源”, “都接新的接口了，就把老的代码全部删掉”

## 1. Problem

`magic-tdx-rs` currently advertises `HistoricalBars`, but its associated
record type is the protocol DTO `SecurityBar`. The provider-neutral router
accepts only `HistoricalBars<Bar = magic_market_core::Bar>`. Consequently a
downstream application cannot register Magic TDX in `BarsRouter` and must keep
provider-specific fallback code.

The existing batch also derives `Provenance.source_at` from the first raw
record. TDX returns bars in ascending time order, so this labels the batch with
its oldest rather than latest source time. The protocol parser can stop early
on a truncated or invalid row and return a partial vector as success. Both
behaviours violate strict evidence and atomic-batch requirements.

## 2. Considered interfaces

### A. Change the provider trait output to Core `Bar` — selected

`TdxHqClient`, `TdxSmartClient`, `TdxDirectClient` and `AsyncTdxHqClient`
continue to expose raw protocol methods such as `get_security_bars`, but their
provider-facing `HistoricalBars` implementations return
`DataBatch<magic_market_core::Bar>`. `TdxService::bars` and
`AsyncTdxService::bars` return the same normalized type.

This creates one provider contract, lets `bars_source` accept Magic TDX
directly, and prevents application code from treating a protocol DTO as a
normalized record.

### B. Add `TdxProvider<P>` while retaining raw `HistoricalBars`

This preserves source compatibility but leaves two public provider contracts
for the same operation. Callers can accidentally register or consume the raw
one, and the extra wrapper becomes permanent maintenance surface. This
conflicts with the selected single-interface migration and is rejected.

### C. Convert `SecurityBar` inside the router

This makes the generic router depend on TDX protocol types and encourages
field mixing. It violates the provider boundary and is rejected.

## 3. Public boundary

The normalized provider contract is:

```rust
impl HistoricalBars for TdxHqClient {
    type Bar = magic_market_core::Bar;
    type Error = TdxError;
}
```

The same associated type applies to smart, direct and async clients. The raw
`SecurityBar` type and `get_security_bars` remain protocol APIs because the
normalized implementation must decode real TDX packets; they are not an
alternative provider contract. No downstream production route may depend on
raw DTO fields after its Gateway migration.

## 4. Data flow and units

```text
TDX TCP packet
  -> strict protocol decode (all declared rows or error)
  -> Vec<SecurityBar> raw protocol records
  -> validate request cardinality, timestamps, order, numerics and jumps
  -> normalize every row to magic_market_core::Bar
  -> DataBatch::strict with one batch_id and latest source_at
  -> magic_market_router::bars_source
  -> downstream ReviewEvidenceGateway
```

For each normalized record:

- instrument and interval come only from the validated `BarsRequest`;
- TDX `adjust=0` maps only to `Adjustment::Unadjusted`;
- OHLC pass through checked `Price::new`;
- `SecurityBar.vol` is source shares and is divided by 100 at the Provider
  boundary to produce Core `Quantity` in lots;
- `SecurityBar.amount` is CNY yuan and maps unchanged to `Money`;
- provider is `ProviderId::Tdx`;
- record batch ID equals batch provenance batch ID;
- daily/weekly/monthly/yearly bar time stays `YYYY-MM-DD`;
- intraday `bar_start`/`bar_end` canonicalize source `YYYY-MM-DD HH:MM`
  to Core `YYYY-MM-DD HH:MM:00`;
- record `source_at` retains the exact source precision and never substitutes
  local fetch time.

The unit admission probe verifies positive-volume rows with:

```text
vwap = amount_yuan / volume_shares
```

and requires VWAP to lie within the bar low/high range within a documented
rounding tolerance. This unit was proven by the 2026-07-24 live probe:
`600396` returned `volume_shares=291,485,664`,
`amount_yuan=4,508,951,040`, giving `vwap≈15.47` inside
`low=14.77/high=16.41`. Treating the raw value as lots gives an impossible
`vwap≈0.1547`. The check is evidence only; it never fills or changes a record.

## 5. Strict failure policy

The whole batch fails explicitly for any of the following:

- empty response or decoded cardinality above the requested limit;
- packet-declared row count not fully decoded;
- missing/empty provenance source or batch ID;
- invalid calendar components or disagreement between components and
  `datetime`;
- duplicate or non-increasing timestamps; records are never sorted or
  deduplicated silently;
- non-finite fields, non-positive prices, negative quantity/amount, or
  inconsistent OHLC;
- positive quantity with zero amount;
- adjacent close change beyond ±20% without same-batch corporate-action
  evidence; the error says manual confirmation is required;
- a source time newer than local observation time;
- an adjustment other than the actually requested `adjust=0`.

For a “latest N bars” request, sequence continuity means the source-returned
bar sequence is complete, strictly ordered and has the requested cardinality.
Wall-calendar gap classification is not guessed: weekends and exchange
holidays require an official trading calendar, which is a separate capability.

## 6. Provenance

One local observation time and one generated non-empty batch ID are created
after a successful raw response. Every Core `Bar` carries that batch ID and
`ProviderId::Tdx`. Batch provenance carries:

- source: `tdx`, `tdx-smart`, `tdx-direct`, or `tdx-async`;
- source_at: latest normalized source timestamp;
- fetched_at: local observation timestamp;
- batch_id: exactly the value carried by every record.

No TDX batch may combine records from another provider. Router failover selects
one complete batch, never field-level fallback.

## 7. Failure modes and rollback

| Failure | Behaviour |
| --- | --- |
| TDX transport/server unavailable | typed TDX error; router may try the next complete provider |
| packet truncated/invalid | atomic TDX failure; no partial batch |
| record validation fails | atomic TDX failure with offending code/time/field |
| unit evidence fails | TDX bars capability is not admitted |
| router rejects evidence | attempt is recorded as rejected; no mixed batch |

Rollback is `git revert` of the implementation commit. The raw TDX protocol
methods remain available for diagnostic probes, so rollback does not require a
protocol or wire-format migration.

## 8. Old-module disposition

| Module/API | Decision | Reason |
| --- | --- | --- |
| raw `get_security_bars` | adopt internally | real TDX transport and parser |
| `HistoricalBars<Bar = SecurityBar>` | delete/replace | leaks protocol DTO and cannot enter router |
| `TdxService::bars -> SecurityBar` | delete/replace | duplicates normalized provider contract |
| downstream provider-specific fallback | delete after Gateway migration | router owns whole-batch failover |
| RustDX source and aliases | reject/delete | user-selected Magic TDX only |

## 9. Verification

Gate B requires unit and router tests for all mappings and failure paths. Gate C
requires formatting, Clippy with warnings denied, all tests, compliance and
documentation checks. Gate D requires:

- deterministic Magic TDX critical-line coverage at least 95%;
- a real Shanghai/Shenzhen/Beijing daily-bar probe;
- a real intraday probe when the source supplies a completed interval;
- source/batch/record evidence assertions;
- router selection and failover evidence;
- downstream `monitor --review` and normal `monitor` logs showing the new
  Gateway route rather than the deleted local fallback.
