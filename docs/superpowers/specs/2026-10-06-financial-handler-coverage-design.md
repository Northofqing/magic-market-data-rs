# Financial handler input and fiscal period coverage

Date: 2026-10-06
Gate A: offline behavior tests within the verified standing development and
feature-submission scope. No new runtime or Provider authority is inferred.

## Actual baseline

Base f57c114190436e6ec96faa60910c127925ffbcfc. Original workflow 37446154439
measured overall 69302/81598 (84.93%, passes 80), critical 35372/39817
(88.84%, fails 95), checker exit 1. Original JSON SHA256
bfbe3aafd8b21e4a30458a5045e36d93ce98c0f05059bfce684dd248e43abbc9.
The original audit failed fetching the RustSec database; only that job was
retried, and audit job 112328508220 passed without remeasuring coverage.
Both original failure and retry remain preserved outside the frozen packets.

The unchanged checker functions reproduce the original integers with only a
read-only in-memory CI-root projection. grpc_production.rs still has 1884
missing production lines. Generic zero-call regions are investigation clues,
not a count of unique uncovered lines. This slice targets the actual generic
execute_financial_statements entry point, not a copied request decoder or
financial projection and not an artificial full-coverage promise.

## Narrow implementation

Append tests to the existing external handler module, retaining its complete
50367-byte prior Git prefix (38 tests/helpers) and the earlier 61-test files.
Production files, interfaces, dependencies and Provider admission are unchanged.

- Exercise accepted schema versions 1 and 2 with Balance, Income and CashFlow.
  A recording offline FinancialStatements implementation must receive the
  original ordered instruments and statement kind exactly once. Run the actual
  handler and projection; preserve batch identity and source/observation fields.
- Check schema 1 omits fiscal_period while schema 2 preserves the exact H1/FY
  source labels, financial lines, source nulls and each record's original
  evidence. Under BR-064, report dates alone must not create a missing label,
  currency, announcement date or numeric value; a period label does not prove
  cumulative-versus-single-quarter semantics.
- Invalid versions, schema names, missing or wrongly typed required fields,
  unknown fields and wrong JSON shapes must fail as InvalidRequest before the
  recording Provider is called. Use valid canonical JSON and exercise the
  handler's actual typed decoder rather than failing fixture construction.
- Concrete HITHINK HTTP/transport/response errors must remain the exact existing
  structured ServiceError failures, retaining FinancialStatements operation
  identity and redaction. No error may turn into successful empty records.
- A second oversized financial record rejects the complete result; establish
  independently that the first fits and the second exceeds the same bound.
  At the pure projection boundary, partial batch quality remains complete=false;
  this is not an assertion that a partial native Provider response is admitted.

The fixture is synthetic and makes no HTTP, credential, environment or native
Provider call. These are offline handler contracts, not live financial data,
report/disclosure-news coverage or Mac native Financial approval.

## Gates B through D

Run formatting, focused/affected/workspace tests, Clippy, rustdoc, doctests,
compliance, docs and diff checks on the exact changed source; preserve failures.
Review the fixed f57c-to-new-HEAD source separately for Standards and Spec.
Nonforce-publish the existing feature branch under standing authority, then
check for an existing same-HEAD workflow before one original-workflow dispatch.
Preserve raw logs, JSON, ZIP, terminal state and exact source identities.

Original 80/95 thresholds, critical globs, checker, source JSON, transport policy,
formal 4e, Mac R2 and sealed packets remain unchanged. The 2455-line deficit
assumes the denominator does not change. No runtime candidate, release tuple,
deployment or source-availability claim until the actual release gates qualify.
