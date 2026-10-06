# Provider failure classification coverage

Date: 2026-10-06
Gate A: offline tests under verified standing development and feature-submission
authority. This extends the existing coverage repair plan, not runtime authority.

## Actual baseline and hypothesis

Base 19a2c677edabde4915dbf738cea380cf967a7345. Original security workflow
37442001456 measured overall 69160/81598 (84.76%, passes 80), critical
35230/39817 (88.48%, fails 95), checker exit 1; audit passed. Original JSON
SHA256 470792d19f2671bdff4938358fdc2e96353cdb4b7f5a3f1b5deeb9567e42b013.
The untouched checker functions reproduce these counts with only a read-only
in-memory CI-root projection. This is the fast failing baseline, not new coverage.

grpc_production.rs still has 2026 missing measured lines. The previous five
news-evidence tests covered 60 additional critical lines. The next hypothesis is
that concrete provider-error downcasts and mapper branches lack service-boundary
tests. Inspect the existing mapper and public provider error enums, then exercise
that actual pure production seam; do not duplicate its classification algorithm
inside a fixture. Generic zero-call regions are clues, not unique missing lines.

## Narrow behavior slice

Append offline tests to the existing external handler-behavior module. Preserve
its complete prior 14-test/helper byte prefix and the earlier 61-test files.
No production source or interface changes are required.

- Invoke provider_error with concrete error types, checking exact ServiceError
  category, operation and reason. Invalid requests and unsupported scopes retain
  the raw supplied reason; legacy transport and response failures retain the
  provider error display where that existing contract requires it.
- Check authentication, rate limiting, HTTP 499/500 boundaries, schema drift,
  incomplete responses, TLS and explicit verified-empty/probe errors separately.
  An error result must not turn into successful or complete empty data.
- For CLS and HITHINK, assert provider identity, stable failure kind and exact
  structured reason, including HTTP 401/403/429/499/500/599/600 and business-code
  categories. HITHINK transport/decode/protocol details remain redacted.
- Check Consensus-specific invalid evidence at its existing typed boundary and
  the conservative unknown-error fallback. Use literal expected categories and
  named cases, not a second mapper or a real credential/provider call.

These tests establish offline error-contract behavior only. They do not prove
that a native technology-news, financial-report or disclosure source is admitted,
available, complete or capable of finding Rubin/Muse.

## Gates B through D

Keep production functions, dependencies, transports, credentials, admission,
original JSON/checker, thresholds, globs, ignores and formal 4e unchanged.
Run formatting, focused/affected/workspace tests, Clippy, rustdoc, doctests,
compliance and documentation checks on the changed source. Preserve failures.
Review Standards and Spec independently at fixed base/new HEAD. Nonforce-publish
the existing feature branch and reuse the original security workflow for one
exact-HEAD measurement, checking for an existing run before dispatch.

The remaining 2597-line deficit assumes an unchanged denominator. No promised
coverage gain, runtime candidate, release tuple or formal switch until actual
release gates qualify. Existing frozen source and CI packets remain immutable;
new source, checks, reviews and CI results receive separate shared packets.
