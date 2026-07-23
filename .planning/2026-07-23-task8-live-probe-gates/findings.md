# Findings

- Existing design requires truthful, non-empty live evidence but lacks a
  reusable machine admission validator and typed empty result.
- THS currently converts “no institution estimate” into one incomplete
  `ConsensusSnapshot`, allowing the live probe to look successful.
- Several probes print configured pacing instead of measuring request starts.
- Touched inline test bodies must move to path-based test sources rather than
  widening APIs for integration tests.

