# Magic TDX Normalized Historical-Bar Pagination

**Date:** 2026-07-28  
**Status:** Gate A design ready  
**Business rules:** BR-002, BR-022, BR-036

## 1. Problem

The normalized `HistoricalBars` contract accepts a positive `u16` limit, but
the TDX wire operation accepts at most `MAX_KLINE_COUNT=800` rows per request.
The current adapter sends the normalized limit directly to one wire request.
Requests for 801 or more rows therefore cross a protocol limit even though the
public contract advertises them as valid. Downstream delayed-outcome windows
cannot reliably request enough source history through the normalized Provider.

The raw blocking and asynchronous clients contain pagination helpers, but that
behavior is not shared by Smart, Direct and deterministic decoded-query paths.
The async raw helper also appends older pages after newer pages, which is not
the strictly increasing provider order required by normalized Core bars.

## 2. Decision

Keep one public interface:

```rust
HistoricalBars::historical_bars(&BarsRequest) -> DataBatch<Bar>
```

Hide exact paging inside the normalized adapter. Blocking, Smart and Direct
Providers use one private blocking implementation; the async Provider uses the
equivalent private async implementation. Both issue protocol-sized
`security_bars` calls through the existing decoded-query seam.

For a requested limit `N`:

1. request `min(remaining, 800)` rows at offsets `0, 800, ...`;
2. require every page to contain exactly the requested page cardinality;
3. retain every complete decoded page until all `N` rows are present;
4. reverse page order while preserving the source order inside each page,
   because a larger TDX offset addresses an older page;
5. pass the one complete sequence to the existing atomic `normalize_bars`;
6. create provenance and record batch identities only after every page and
   every normalized record has passed validation.

No page is exposed as a successful partial batch. No mock, alternate Provider,
sorting, deduplication, filling or field-level fallback is introduced.

## 3. Interface and module depth

`BarsRequest` remains unchanged and callers do not learn page sizes, offsets or
wire ordering. The decoded-query seam remains crate-private and continues to
return decoded `SecurityBar` records. Pagination, cardinality, ordering and
atomic failure stay local to `adapter.rs`, where all four Provider paths
already converge.

Date-range requests remain explicitly unsupported. This change makes the
existing latest-`N` contract honor its full `u16` limit; it does not silently
reinterpret `start` or `end`.

## 4. Data flow and evidence

```text
validated BarsRequest(limit=N, unadjusted)
  -> exact protocol pages through the selected Magic TDX client
  -> every page succeeds with exact requested cardinality
  -> older pages precede newer pages, source row order preserved
  -> one complete sequence
  -> existing identity/time/OHLC/volume/amount/order validation
  -> one DataBatch<Bar> with one source label, observation and batch ID
```

Every record remains `ProviderId::Tdx`, `Adjustment::Unadjusted`, carries its
exact provider bar time and shares the batch provenance identity. An error on
any page prevents provenance creation and rejects the whole operation.

## 5. Failure modes

| Condition | Required behavior |
| --- | --- |
| limit `1..=800` | one exact wire request |
| limit `801..=u16::MAX` | exact protocol-sized pages until the limit is satisfied |
| empty or short page | reject the whole request with explicit cardinality error |
| oversized page | reject the whole request with explicit cardinality error |
| transport/parser error on any page | return failure; no partial `DataBatch` |
| duplicate or non-increasing time within/across pages | existing atomic normalization rejects |
| invalid timestamp, OHLC, volume or amount on any page | existing atomic normalization rejects |
| structurally valid adjacent move above 20% | preserve under BR-022; downstream policy decides |
| unsupported range | reject before transport, unchanged |

## 6. Old-module relation

| Existing module | Decision | Reason |
| --- | --- | --- |
| raw `get_security_bars` | adopt | real bounded wire operation |
| raw blocking `get_security_bars_all` | retain as raw API | diagnostics may use it; normalized contract owns stricter atomic semantics |
| raw async `get_security_bars_all` | retain as raw API | not used as normalized ordering evidence |
| `BlockingTdxQuery` / `AsyncTdxQuery` | adopt | one deterministic seam for all Provider paths |
| one-call normalized adapter | replace | violates valid normalized limits above 800 |
| downstream Provider-specific pagination | reject | wire knowledge belongs behind the Provider interface |

## 7. Validation

Behavior tests exercise the normalized decoded-query path for:

- exact 800 and 801 row boundaries;
- more than two pages with exact total cardinality and ascending output;
- a second-page error rejecting the whole request;
- short/empty page rejection;
- duplicate, reversed, invalid timestamp and structurally invalid records
  across page boundaries;
- matching blocking and async call offsets/counts.

Gate C runs formatting, focused/full crate tests, check, Clippy with warnings
denied, workspace tests, compliance and documentation links. Gate D retains
the repository coverage and bounded live-probe requirements.

## 8. Rollback

Revert the implementation commit and this Gate A registration together:

```bash
git revert <commit-sha>
```

Rollback restores the prior 800-row normalized ceiling. It must not remove
BR-022 structural validation or alter downstream jump-confirmation policy.
