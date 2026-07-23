# Findings

- Existing design requires truthful, non-empty live evidence but lacks a
  reusable machine admission validator and typed empty result.
- THS currently converts “no institution estimate” into one incomplete
  `ConsensusSnapshot`, allowing the live probe to look successful.
- Several probes print configured pacing instead of measuring request starts.
- Touched inline test bodies must move to path-based test sources rather than
  widening APIs for integration tests.
- Core `DataBatch<T>` exposes records, provenance, and quality; `Provenance`
  stores source/source_at/fetched_at/batch_id, but no common record-evidence
  trait exists. The verifier therefore needs evidence and identity closures.
- THS `consensus()` currently accumulates an issue and pushes a snapshot with
  empty estimates, then returns `DataBatch::best_effort`; the live example
  accepts any `Ok` and prints it.
