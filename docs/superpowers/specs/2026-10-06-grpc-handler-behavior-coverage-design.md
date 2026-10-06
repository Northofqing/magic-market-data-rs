# gRPC handler behavior coverage

Date: 2026-10-06
Gate A: a narrow source/test repair within the verified standing development
and feature-submission authorization. No deployment authority is added.

## Measured baseline

Base cfdb27683cd06ed51fbfb1c8f4c129067b21c935, security workflow run
37430519677, coverage job 112160058874. The original checker measured overall
68716/81598 (84.21%, passes 80) and critical 34787/39817 (87.37%, fails 95).
The original report SHA-256 is
1f9122ca8fe5e160b7455b0d5f3ce29d776f127498d1515a2fe6c6acc35645d1.
Audit passed; the critical failure and original artifacts remain immutable.

The original critical set has 115 measured files. grpc_production.rs has
2469 missing lines, followed by derived_products.rs (230), the TDX adapter
(175), Router adapters (154), and Eastmoney Miaoxiang (146). This is code-test
coverage, not news-source completeness. The 3040-line deficit assumes an
unchanged denominator and is not a promise of the next result.

## First behavior slice and seam

Use the existing public OperationRegistry::execute boundary. Add an external
unit-test module; leave all 61 previously extracted tests and their helpers
byte-identical. No production function body or interface changes.

- Exercise registered, admitted, runtime-available handlers with an explicitly
  pinned provider and an invalid schema. They must reject the envelope rather
  than call a source with it. Use no valid source query in this matrix.
- Verify diagnostic registrations reject ordinary access without opt-in.
- At the existing NewsProvider trait boundary, supply a deterministic offline
  fixture. Verify request bounds/schema rejection before provider invocation,
  complete v2 source evidence, typed rate/auth/outage/rejection failures,
  transport-detail scrubbing, atomic evidence conflict and payload-bound refusal.

Assertions use independent literal contract examples, not copied algorithms or
snapshots computed from implementation. Fixture invocation counters establish
the no-provider-call boundary for malformed news requests. Fixtures cannot
prove native transport, source admission, live RPC, full history or production
availability. The matrix's invalid-schema cases do not constitute live probes.

## Gates B through D

Only a cfg(test)/path declaration and new test/design files are in this slice.
Do not change checker, thresholds, critical globs, ignores, dependencies, source
allowlists, transport policy, admission, authentication or runtime controls.

Run targeted tests, formatting, affected library tests, Clippy, documentation
and compliance checks appropriate to the new source. Preserve every failure.
Review Standards and Spec independently at a fixed base/new HEAD, publish only
the existing feature branch without force, and measure the same new HEAD using
the original security workflow and original 80/95 checker. Do not claim a
coverage gain from test counts alone. Any discovered production defect needs
a separately stated reproducer and minimal justified fix.

No new runtime candidate or SDK release tuple is generated before actual
release gates qualify. The running 4e service, existing eea/Mac R2 artifacts,
authentication, data owners, listener, start/stop and keepalive remain unchanged.
