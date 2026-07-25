# Progress

## 2026-07-25

- Read upstream engineering rules and applicable skills.
- Audited existing Core, CNInfo, official-exchange and Router announcement
  paths.
- Proved CNInfo has a native empty-stock market-list protocol.
- Presented three approaches; parent approved native CNInfo option A.
- Registered BR-018.
- Wrote Gate A design and TDD implementation plan.
- Core, CNInfo Provider, Router adapter/policy and bounded probe are implemented
  with focused tests.
- First production probe reached CNInfo and exposed its nonstandard
  `totalpages=floor(total/pageSize)` metadata. Recorded the raw values and
  corrected the strict validator without removing any completeness check.
- Corrected pagination validation passed all focused Core, CNInfo and Router
  tests, example-probe tests, scoped rustfmt, strict Clippy and diff checks.
- The bounded production probe for `2026-07-24`, limit `3`, admitted three
  records from a complete `total=1108` source batch with provider publication
  times and IDs preserved.
- R-08 upstream slice is ready for integration; repository-wide release gates
  remain the parent/integrator's responsibility because this shared worktree
  contains concurrent unrelated changes.
