# Eastmoney limit-pool completeness implementation plan

1. Register BR-028 and approve the source-total admission design.
2. Add RED tests for complete, truncated, verified-empty, invalid total, and
   duplicate identity responses.
3. Add a provenance-preserving batch construction path for strict empty and
   explicit incomplete quality.
4. Parse and validate `data.tc`, reject duplicate identities, and apply the
   BR-028 outcome matrix.
5. Run focused tests, formatting, Clippy, live probe, and full release gates.
