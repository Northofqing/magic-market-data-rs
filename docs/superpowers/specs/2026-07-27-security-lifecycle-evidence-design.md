# Security lifecycle evidence

**Date:** 2026-07-27
**Rule:** BR-032
**Status:** Gate A approved

## 1. Problem

Downstream daily-bar validation must explain legitimate price discontinuities
without disabling bad-data protection. The released providers do not currently
offer an admissible lifecycle contract:

- TDX exposes `FinanceInfo.ipo_date` and raw `XdXrInfo`, but those DTOs do not
  carry `InstrumentId`, record evidence or atomic batch provenance.
- `SecurityMetadata` has a `listed_on` field, but the TDX, Tencent and Sina
  adapters leave it unavailable.
- Eastmoney exposes typed dividend plans, but that contract does not cover the
  full TDX corporate-action family and its current bounded query cannot prove a
  complete lifecycle result set.

Local observation time, security-code prefixes and downstream mutable caches
are not source evidence. They may not be used to declare an IPO window or an
ex-rights date.

## 2. Decision

The provider workspace will expose one provider-neutral, evidence-preserving
security-lifecycle contract:

1. TDX security metadata is enriched with a strictly decoded listing date from
   the matching finance packet.
2. Core gains a normalized `CorporateAction` record and
   `CorporateActionsProvider` trait.
3. TDX is the first corporate-action implementation. Eastmoney may join the
   same router only after its query proves complete pagination and exact empty
   results.
4. Every downstream exception is derived from an admitted lifecycle batch.
   Raw DTOs and mutable `mark_ipo`/`mark_ex_rights` registries are not public
   production seams.

This is narrower than a general fundamentals redesign. It models only facts
needed to validate historical price continuity.

## 3. Alternatives

### 3.1 Downstream wraps raw TDX values

Rejected. It would invent batch identity and record provenance outside the
provider boundary and allow downstream code to bypass normalization.

### 3.2 Only consume Eastmoney dividend plans

Rejected as the complete solution. It can eventually be a fallback, but it
does not prove IPO dates or the full split/reverse-split/XDXR family.

### 3.3 Disable the adjacent-change gate

Rejected. It would allow corrupt, misadjusted or identity-mismatched series to
enter calculations and contradict downstream data red line 2.3.

## 4. Public contract

### 4.1 Listing evidence

`SecurityMetadataProvider` remains the canonical listing-date contract. For
each requested TDX equity:

- the security-list identity and finance-packet identity must exactly match the
  requested `InstrumentId`;
- `ipo_date` must be an eight-digit, calendar-valid `YYYYMMDD`;
- zero, malformed and future dates are explicit issues or protocol failures,
  never guessed values;
- a valid date is stored as `SecurityMetadata.listed_on`;
- local acquisition completion is `observed_at`;
- `source_at` remains absent unless the protocol supplies a separately
  validated source update timestamp;
- all records and provenance share one non-empty batch ID.

Consumers may project the listing sub-contract from a best-effort metadata
batch, but only after exact identity, cardinality, record evidence and
`listed_on` validation. Missing unrelated price-limit metadata does not erase a
valid listing fact.

### 4.2 Corporate-action types

Core adds:

```rust
pub enum CorporateActionStatus {
    Implemented,
    Proposed,
    Cancelled,
    Unknown,
}

pub enum CorporateActionTerms {
    Distribution {
        cash_per_share: Option<FiniteNumber>,
        bonus_per_share: Option<FiniteNumber>,
        rights_per_share: Option<FiniteNumber>,
        rights_price: Option<Price>,
    },
    Split { ratio: Ratio },
    ReverseSplit { ratio: Ratio },
}

pub struct CorporateAction {
    instrument: InstrumentId,
    effective_on: IsoDate,
    status: CorporateActionStatus,
    terms: CorporateActionTerms,
    evidence: SourceEvidence,
}

pub trait CorporateActionsProvider {
    type Error: Error + Send + Sync + 'static;

    fn corporate_actions(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<CorporateAction>, Self::Error>;
}
```

All amounts are finite and non-negative. Split ratios are finite, positive and
not one. Only `Implemented` records with a source-backed effective date may
explain a historical price discontinuity.

## 5. TDX mapping

The TDX adapter maps only categories whose source semantics are documented:

- category 1 becomes one `Distribution` action and may preserve cash,
  bonus/transfer, rights quantity and rights price together;
- category 11 becomes `Split`;
- category 12 becomes `ReverseSplit`;
- unknown categories remain explicit quality issues or an unsupported mapping
  and cannot authorize a continuity exception.

The parser must reject truncated records, invalid dates, non-finite values and
identity disagreement. It must not stop at a bad row and return a partial
success. Records sort by effective date ascending and stable action kind.
Duplicate `(instrument, effective_on, terms kind)` identities fail the atomic
batch. A protocol-proven zero-row response returns a complete empty batch with
provenance; transport or parse failure does not.

TDX does not supply a provider publication time for these packets.
`SourceEvidence.source_at` therefore remains absent. `effective_on` and
`observed_at` must never be copied into that field.

## 6. Eastmoney fallback boundary

Eastmoney joins the corporate-action router only after all of the following are
proved:

- the source total and pagination are complete before caller limiting;
- exact zero is represented as a complete empty batch with provenance;
- free-text progress is mapped to the normalized status inside the adapter;
- instrument/date/range/duplicate ordering checks match the Core contract.

Until then, Eastmoney dividend plans remain a separate capability and are not
relabeled as complete corporate-action history.

## 7. Downstream admission

`stock_analysis` adds a deep `SecurityLifecycleGateway` that privately
constructs:

```text
AdmittedSecurityLifecycle
  instrument
  listed_on?
  implemented actions ordered by effective date
  listing BatchEvidence
  corporate-action BatchEvidence or verified-empty evidence
```

Admission requires complete requested identity coverage, matching provider and
batch evidence, valid source/observation ordering, deterministic action order,
no duplicate action identity and exact requested date range.

The historical-bars flow obtains bar and lifecycle batches independently,
records both acquisition outcomes, and passes the admitted lifecycle context
into daily-series validation. An IPO exception applies only during the first
five exchange trading days derived from `listed_on`. A corporate-action
exception applies only when an implemented action has
`effective_on == current_bar.date`. Proposed, cancelled, unknown, mismatched,
unavailable and stale lifecycle evidence cannot explain a jump.

The existing mutable global IPO/ex-rights registries are deleted from
production. Tests use explicit fixture lifecycle contexts rather than global
state.

## 8. Failure modes

- Missing listing date: do not claim an IPO window.
- Corporate-action verified empty: no action exception exists for the range.
- Corporate-action unavailable/partial/conflicting: reject a jump that would
  require action evidence; do not convert the result to verified empty.
- Future listing/action date: protocol/admission failure.
- Metadata or action identity mismatch: reject the whole lifecycle batch.
- Audit persistence failure: no lifecycle batch becomes consumer-visible.
- Provider network failure: retain typed retryability; never fall back to
  local inference.

## 9. Tests and release evidence

Upstream deterministic coverage includes:

- valid, zero, malformed and future listing dates;
- finance request/response identity mismatch;
- TDX categories 1, 11 and 12;
- unknown category non-admission;
- invalid date, ratio, amount and truncated packet;
- exact empty, complete non-empty, duplicate and ordering failures;
- record/provenance provider and batch mismatch;
- router fallback only between providers that implement the same contract.

Downstream coverage includes:

- no listing evidence does not activate IPO handling;
- exact first-five-trading-day boundaries;
- implemented same-day action admits the explained discontinuity;
- proposed/cancelled/unknown/mismatched actions do not;
- verified empty and unavailable remain distinct;
- bar and lifecycle evidence are both auditable;
- source guards reject raw TDX lifecycle access and mutable marker APIs.

Release requires workspace formatting, strict Clippy, all-target/all-feature
tests, compliance, documentation checks, unchanged coverage thresholds and
bounded live probes for one known listing date and one known implemented
corporate action. The new contract is published through a new immutable merge
revision; downstream pins every Magic crate to that same revision.

## 10. Rollback

Core, provider/router and downstream adoption are separate commits. Rollback is
`git revert <commit-sha>` in dependency order. If live admission or downstream
release evidence fails, lifecycle capability remains unavailable and
historical jumps requiring it fail closed. Rollback never reactivates raw DTO
access or mutable evidence caches.
