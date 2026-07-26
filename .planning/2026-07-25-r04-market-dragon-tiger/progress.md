# Progress

## 2026-07-25

- Read repository instructions and applicable design/TDD/planning skills.
- Inspected Core, Eastmoney and Router dragon-tiger contracts.
- Ran bounded real Eastmoney schema probes for discovery and buy-side seats.
- Recorded design, implementation plan, and BR-015.
- Core RED failed on missing public types as expected; GREEN now passes for
  bounded request and exact 5+5 disclosure validation.
- Eastmoney discovery RED/GREEN now preserves distinct `TRADE_ID` reasons,
  collapses exact duplicates, rejects conflicts, and sorts before limit.
- Atomic market seat retrieval and the legacy per-instrument seat path now
  filter by exact `TRADE_ID`; `cargo test -p magic-eastmoney-rs` passes 94/94.
- Router RED/GREEN adds a market-disclosure chain with non-empty, limit, date,
  identity, evidence and canonical-order validation.
- The first real probe exposed convertible-bond rows in the broad source
  report. The operation now uses the source's explicit A-share equity type
  filter instead of guessing asset class from names or code prefixes.
- The second real probe exposed record/batch `source_at` granularity mismatch.
  Source evidence is now one exact ISO trading date across entry, seats and
  provenance, with a Core invariant covering source time and observation time.
- The final 2026-07-22 real probe was admitted with five records. Both 002396
  entries survived under distinct `TRADE_ID` values; every record had exactly
  ten seats.
- A further duplicate-row negative test originally used seat name as identity;
  the real probe proved that two legitimate anonymous institution seats can
  both be named `机构专用`. Exact duplicate detection now includes side, name
  and every normalized numeric field, so different source amounts remain
  distinct while a byte-equivalent normalized row fails.
- Final gates: Core tests pass; Eastmoney 95/95 pass; Router tests pass; target
  Clippy with `-D warnings`, target rustdoc, compliance, docs links and
  `git diff --check` pass.
- Integrated workspace Gate D now passes: workspace fmt, strict all-target
  Clippy, all-feature tests, compliance, documentation links and diff checks.
- The final `2026-07-24` production probe was admitted with five disclosures,
  exact distinct entry IDs, ten seats per disclosure and source-backed net
  amounts. No display-level code/name deduplication was applied.
