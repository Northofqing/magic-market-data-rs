# Explicit diagnostic partial-data access implementation plan

1. Extend the versioned Protobuf request/response with an opt-in flag and a
   diagnostic blocker while preserving all field numbers and default behavior.
2. Carry the opt-in bit through `QueryCommand`, add a distinct diagnostic
   registration path, and force diagnostic results to `UNADMITTED` and
   incomplete.
3. Register bounded existing-provider diagnostics for technical bars, fund-flow
   series/snapshots, futures delivery, post-close flows and market rankings.
4. Keep auctions and breadth fail-closed, with tests proving no Provider I/O.
5. Update the external integration guide and README capability counts/status.
6. Run format, unit/integration tests, Clippy, compliance, docs checks and a
   release gRPC smoke test before describing the change as complete.
