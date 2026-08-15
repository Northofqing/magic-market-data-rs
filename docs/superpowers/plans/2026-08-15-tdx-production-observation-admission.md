# TDX Production Observation Admission Plan

**Status:** complete
**Design:** `docs/superpowers/specs/2026-08-15-tdx-production-observation-admission-design.md`

1. Promote the three independently evidenced LocalTerminal constants and update
   admissions evidence; keep source-record count and all LocalAnalysis constants
   false.
2. Preserve field-level admission through monitor output and promote only valid
   raw observation envelopes in the Windows Agent.
3. Revalidate admitted payload family, schema, identity and value in the gRPC
   server; advertise exact admitted families in listener status.
4. Run three serial fast reads, three serial amount snapshots, targeted tests,
   Clippy, compliance and a real gRPC production replay.
5. Update README, deployment and external integration documentation.
