# Production registry preflight limit coverage

Date: 2026-10-07

## Gate A scope

Narrow test-only continuation of the verified standing development repair.
Use the existing `production_operation_registry` public constructor and the
existing external handler test module. No new seam, injection, production body,
architecture, contract, source admission, transport or dependency change.

The original exact-HEAD CI for 7174f09f0b34d082bc584180ba4260d53f379d6c,
run 37513568982, remains failed at critical 35392/39817 (88.89%, required 95%).
Do not rewrite its JSON, checker, thresholds, globs, receipt or old source hashes.

## Behavior slice

The real production constructor checks configuration before constructing a
Tencent client or registering providers. Cover these existing typed outcomes:

- zero provider timeout returns `InvalidLimit` with its exact timeout reason;
- zero payload ceiling with a positive timeout returns `InvalidLimit` with its
  exact payload reason;
- both limits zero retain the existing timeout-first validation order.

These calls return before any provider client is constructed. They do not read
credentials, query a provider, bind a listener or invoke RPC. Do not use a fake
registry or assert only that an arbitrary error occurred.

## Evidence limits

The original merged source segments identify the two preflight bodies as zero
count. A read-only projection's unique-line denominator does not equal every
original per-file summary; its first strict equality assertion is preserved as
a failure. That projection is a branch-selection aid only, not new coverage or
a prediction that these tests achieve 95%. A fresh exact-commit original CI is
required before claiming any release gain.

## Gates B through D

Preserve all old tests and assertions; add only the three preflight tests. Run
targeted and affected-library tests, formatting, strict Clippy, documentation,
compliance and link checks. Record original-command failures explicitly. No
feature publication, CI dispatch, release tuple or running-service upgrade is
part of this test-only work-in-progress handoff.

The independently captured native Bash failure has an oversized XSAVE stack
allocation mechanism and a standalone CPUID reproducer. These Rust tests do
not repair that runtime. The checker and installed DLL remain unchanged.

Rollback: remove only these new tests and this design. Preserve original CI,
native fault captures, user files and the formal gRPC service.
