# Progress

## 2026-07-23

- Restored the previous project state and preserved the user's untracked
  integration requirements document.
- Audited the root README, all provider capability documents, router contract,
  deployment runbook, crate READMEs, workspace manifest and toolchain.
- Selected the operator-first entry-manual design under the user's standing
  no-confirmation instruction.
- Wrote the README design and opened a dedicated persistent plan.
- Wrote the exact implementation plan with required headings, capability truth,
  commands, verification gates, package checks and delivery steps.
- Replaced the 34-line root README with a 477-line Chinese-first entry manual.
- Audited Core and EMQuant implementation details and corrected two subtle
  distinctions: minute K lines are not `MinuteData`, and Bar fetch time lives
  in batch provenance rather than each Bar record.
- Passed documentation link, compliance and diff-whitespace checks.
- Added the README expansion to the unreleased changelog and moved to the full
  Rust 1.83 release gate.
- Passed the isolated Rust 1.83 release preflight: workspace check, all tests,
  strict Clippy, rustdoc/doctests, documentation links, compliance and diff.
- Completed a local pre-commit review because subagent review is prohibited.
  Confirmed that EMQuant is not presented as live-passed, Tencent is not
  presented as an SLA source, TDX Quote source time remains unverified and all
  unsupported boundaries stay explicit.
