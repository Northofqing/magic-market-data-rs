# SEC typed entry metadata coverage

Date: 2026-10-07
Gate A: test-only work within the verified standing development scope.
This design does not grant a new source, runtime, credential or release authority.

## Baseline and scope

The committed base is f57c114190436e6ec96faa60910c127925ffbcfc. Its original
workflow measured critical 35372/39817 (88.84%, fails the original 95% gate)
and overall 69302/81598 (84.93%, passes 80). This slice does not predict a gain
from test count or generic instantiation count.

The existing 11 financial-handler tests remain pending; their complete current
WIP file is retained as a prefix before one test-module declaration is appended. Native
Windows Bash can still terminate with 0xC0000005 in the documentation checker;
the original checker and failures are retained. This independent offline slice
can be developed and tested without declaring that environment issue fixed.
No commit, push, CI dispatch, runtime candidate or formal switch is justified
by a partial check set or by a passing diagnostic replica.

## Existing production seam

SEC registration already calls the generic execute_typed function with the
real CompanyFilingRequest and CompanyFiling types, schema version 1 and the
SecEdgarError mapper. Exercise that same executor and its real decoder and
provider_query_result projection with a recording offline closure. Do not
instantiate a native SEC client, register a new handler or perform HTTP.

Add an external grpc_sec_typed_entry_tests module through the existing external
grpc_handler_behavior_tests file, retaining its complete prior WIP content.
Preserve production functions,
the earlier 61-test files, the financial WIP, schemas, dependencies, admissions,
transport controls, original coverage checker, globs and thresholds.

## Behavior to verify

- Forward normalized ordered companies, original forms, explicit bounded dates
  and max_records exactly once. Also retain valid absent dates and empty forms.
- Preserve every metadata record and its original evidence, links, form,
  accession, report date, accepted_at, batch identity and source/observed times.
  Missing report_period or accepted_at stays missing; a 10-Q is not a synthetic
  half-year report and a filing date is not a report period or source timestamp.
- Reject wrong envelope schema/version and invalid bounded request shapes
  before the closure is called. Exercise the actual public constructors through
  Deserialize, not a copied decoder. Do not assert a nonexistent accession
  request filter or add an unknown-field contract to this test-only slice.
- Each concrete SEC request/authentication/unsupported/decode/protocol/Core
  failure remains its existing ServiceError and cannot become empty success.
  Do not claim new redaction behavior for this legacy mapper.
- A valid first record and oversized second record reject the whole result;
  prove each record's size against the same bound independently.
- The pure projection preserves incomplete quality as complete=false. This
  does not admit partial native SEC retrieval or prove a verified-empty source.

All issuer identities, URLs, dates and records are explicitly synthetic offline
fixtures. Under BR-041 this seam proves metadata handling only, not document
bodies, attachments, XBRL, financial figures or a Mac dispatch implementation.

## Gates B through D

Run focused tests, affected library tests and the documented formatting,
workspace tests, Clippy, rustdoc, doctest, compliance, documentation and diff
checks on the exact changed source. Preserve first failures and terminal logs.
The documentation runtime blocker is explicit and must not be hidden by cache,
changed input content, ignored failures or lowered gates.

Review a fixed source snapshot separately for Standards and Spec before any
feature submission. Reuse or trigger only the original workflow for the exact
new HEAD after repository gates qualify. Keep the old formal server, agent,
authentication and Mac R2 unchanged. Real business acceptance and release
qualification require their own matching identities and evidence.
