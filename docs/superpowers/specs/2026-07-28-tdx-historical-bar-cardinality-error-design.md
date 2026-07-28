# Magic TDX Historical-Bar Cardinality Error Contract

**Date:** 2026-07-28  
**Status:** Gate A design ready  
**Business rules:** BR-022, BR-036, BR-038

## 1. Problem

BR-036 made normalized historical bars page exactly above the TDX 800-row
wire limit. A source with fewer rows than a normalized request is still a
valid and recoverable condition for callers that can accept the exact
available history. The provider currently exposes that condition as
`InvalidData(String)`. Pagination changed the display text from a whole-request
message to a page message, which broke downstream cardinality recovery even
though the underlying evidence was unchanged.

Display strings are not an interface. Parsing them also discards the page
offset needed to distinguish a source with 100 total rows from a source with
900 total rows whose second page returned 100.

## 2. Decision

Add one public `TdxError` variant:

```rust
HistoricalBarCardinality {
    offset: u32,
    actual: usize,
    expected_page: u16,
    requested_total: u16,
}
```

The blocking, Smart, Direct and async normalized pagination paths return this
variant for an empty, short or oversized exact page. The final
`normalize_bars` cardinality guard returns the same variant with `offset=0`,
`expected_page=request.limit()` and `requested_total=request.limit()`.

The error's `Display` implementation remains human-readable, but consumers
must match the variant and fields. They must not parse the display text.

## 3. Data and failure contract

For a rejected normalized request `R`:

1. `requested_total == R.limit`;
2. `expected_page` equals the exact wire count requested at `offset`;
3. `actual` equals the decoded source row count returned for that page;
4. `offset + expected_page <= requested_total`;
5. the available source cardinality may be derived as `offset + actual` only
   with checked conversion/arithmetic and only after validating items 1–4;
6. no page is emitted as a successful normalized batch;
7. no provenance, batch ID, filling, sorting, deduplication or fallback is
   created to conceal the mismatch.

Transport attempts completed before the mismatch are caller-owned audit
evidence. They do not turn the rejected provider request into a partial
success.

Malformed field relationships are provider-invalid evidence and must fail
closed. A caller may make a new exact request for the checked available count;
that is a new audited request, not reuse of the rejected partial pages.

## 4. Old-module relation

| Existing behavior | Decision | Reason |
| --- | --- | --- |
| `InvalidData(String)` cardinality text | replace | unstable and loses typed page identity |
| BR-036 atomic pagination | adopt unchanged | no partial `DataBatch` may escape |
| downstream string parsing | reject | provider contract now exposes typed evidence |
| downstream exact retry | retain | new request can verify the exact available count |

## 5. Validation

Tests must prove:

- blocking and async second-page short responses expose all four exact fields;
- single-page normalization uses the same typed variant;
- empty, short and oversized pages remain atomic failures;
- a downstream request above 800 derives `offset + actual`, verifies it against
  the rejected total, and performs a new exact request;
- malformed/overflowing field relationships are rejected;
- no consumer retains the former cardinality display-text parser.

Gate C remains workspace formatting, tests, strict Clippy, compliance and
documentation checks. Gate D retains bounded live-provider validation and
coverage thresholds.

## 6. Rollback

Revert this amendment and its implementation together. A rollback also requires
reverting any downstream revision pin that matches the structured error
contract. BR-022 and BR-036 atomicity must remain intact.
