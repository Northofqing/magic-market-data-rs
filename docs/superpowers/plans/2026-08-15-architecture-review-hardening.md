# Architecture review hardening plan

1. [x] Bound every TDX decompression path and remove the insecure financial
   HTTP fallback.
2. [x] Restore actual TDX async concurrency; add I/O and response deadlines,
   reconnect, block-code checks and chronological pagination.
3. [x] Complete normalized evidence/status contracts and Router regressions.
4. [x] Seal option invariants, bar spans, evidence timestamps and provenance.
5. [x] Replace handwritten SHA-256 and remove avoidable order-book allocation.
6. [x] Add Agent heartbeats, server idle expiry and bounded TLS file reads.
7. [x] Harden Tencent/Sina endpoint identity, share Eastmoney/MX pacing, and
   avoid production ranking reparse.
8. [x] Complete Gate C: formatting, workspace tests, Clippy, compliance,
   documentation links and diff checks.
