# Permissionless Capability Closure Plan

1. Lock deterministic fixtures for complete Eastmoney ranking pagination,
   source-time skew, three-exchange coverage and fail-closed drift cases.
2. Run two live and three serial ranking probes.  If all Gate C evidence passes,
   register the strict `MarketRankings` provider in the production gRPC registry;
   otherwise retain the current partial diagnostic and record the exact blocker.
3. Move CFFEX formal HTTPS execution onto the existing shared bounded
   reqwest/rustls transport, keep plaintext ureq diagnostic-only, and run two
   formal live plus three serial requests before changing admission.
4. Exercise the public Eastmoney minute/daily fund-flow contract twice live and
   three times serially.  Promote only the exact passing family and keep
   Miaoxiang diagnostics independent.
5. Build the complete breadth acquisition composition from proved universe,
   dynamic quotes and upper/lower pools using `MarketBreadthAnalysis`; add
   coverage/skew request validation and deterministic failure tests before any
   live run.
6. Audit Baidu technical bars against admitted calendar, adjacent TDX daily bars
   and corporate-action evidence.  Extend the contract only if all evidence can
   be retained without cross-provider relabelling.
7. Extend the TDX loopback parser only for response-backed LastClose/OHLC fields,
   with captured fixtures and typed unavailable values.  Do not implement a
   synthetic source-record count.
8. Complete calendar/session reset and versioned rule-configuration seams for
   local anomaly events.  Keep all anomaly admission constants false until a
   trading-day shadow run proves caller-selected production rules.
9. Run formatting, crate/workspace tests, Clippy, compliance, documentation
   checks and live gRPC probes.  Update capability text and admission rows only
   for families whose evidence gate passed; preserve Level-2/CFETS/IMF blockers.
