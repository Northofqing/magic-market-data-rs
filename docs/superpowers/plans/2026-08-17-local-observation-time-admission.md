# Local observation-time admission implementation plan

1. Update BR-019, BR-050 and add BR-053 without weakening BR-033.
2. Make Eastmoney's formal `PostCloseFlows` path use a complete observation-time
   batch while preserving per-record source instants.
3. Register production `PostCloseFlows` and TDX `T0Evidence` handlers; keep
   operation numbers and request contracts unchanged.
4. Add deterministic tests for mixed record source instants, exact local
   observation time, completeness, registration and fail-closed schemas.
5. Complete two live and three serial observations for both admitted paths.
6. Update admissions, deployment and gRPC integration documentation.
7. Run formatting, targeted and workspace tests, Clippy, compliance, docs and
   release checks before replacing the running binaries.

