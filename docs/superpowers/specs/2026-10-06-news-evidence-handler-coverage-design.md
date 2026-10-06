# News evidence handler coverage

Date: 2026-10-06
Gate A: narrow offline test work within verified standing development and
feature-submission authority; no runtime or deployment authority added.

## Measured baseline and feedback loop

Base f9cfb6003f82daf9385155e249fe818dd4dc7a5b. Existing security workflow
37438294242 measured overall 69100/81598 (84.68%, passes 80) and critical
35170/39817 (88.33%, fails 95), original checker exit 1; audit passed.
Original JSON SHA-256:
13e0b4fc85e39dcff4fecc743fd9d087a9cc3a9ab2186871abeaac49488276f0.
The prior slice added 383 covered critical lines without changing either
denominator. grpc_production.rs still has 2086 missing measured lines.

The existing exact report is replayed with the untouched checker line-count
and glob functions, mapping only the precise CI root in memory. It reproduces
the failing integer counts in seconds and is diagnostic, not a new release
measurement. Original JSON, ZIP, checker and threshold stdout remain unchanged.

## Next behavior slice

Reuse the existing offline NewsProvider fixture through the public
OperationRegistry::execute boundary. Append tests without altering any of the
previous nine tests/helpers or the earlier 61 test files.

- Explicit incomplete quality and ordinary empty batches must fail with their
  own literal InvalidEvidence code/field, never become source-proven empty.
- Missing batch identity must fail at the public Provenance JSON boundary;
  Provenance::new generates an identity and cannot construct that missing state.
  Missing batch source time and malformed batch timestamps must fail at the
  handler's batch boundary with no record index.
- Conflicting record batch identity, missing/malformed source time, malformed
  publication/observation time, mismatched publication time, impossible time
  ordering and first-record/batch source mismatch must retain distinct code,
  field and index.
- An invalid or unsorted second record must reject the entire response instead
  of returning the valid first record.
- Equivalent UTC/+08 instants must be accepted while preserving source strings
  exactly; comparison must not rewrite source evidence into invented precision.

Use independent literal cases and invocation counts. No real provider request,
listener, RPC, authentication read or source-admission claim is in scope.

The first run's 12-pass/1-fail fixture assumption is retained: omitting
with_batch_id did not remove the constructor-generated identity. Correct the
test to the actual public boundary, not the production implementation or a
fabricated private state.

## Gates B through D

Production files, interfaces, dependencies, admission, transport controls,
coverage checker, thresholds, globs and ignores remain unchanged. Preserve all
prior failures. Run formatting, focused/affected/workspace tests, Clippy,
rustdoc, compliance and documentation checks on this changed source; review
Standards and Spec separately at fixed base/new HEAD, then nonforce-publish the
existing feature branch and measure the exact new HEAD with existing workflow.

The remaining 2657 covered-line deficit assumes an unchanged denominator and
is not a promise for this slice. No test-count-based coverage claim or runtime
candidate/release tuple before actual release gates qualify. Formal 4e and
old eea/29a/abf/Mac R2 remain unchanged.
