# TDX Dynamic Watchlist Implementation Plan

**Status:** complete on 2026-08-15
**Design:** `docs/superpowers/specs/2026-08-15-tdx-dynamic-watchlist-design.md`

## Task 1 — Contract and rule

- Add BR-047 without changing LocalTerminal/LocalAnalysis admission.
- Add `SetWatchlist`, typed request/response, Agent configure command, and
  desired/applied listener status fields.
- Centralize canonical EQUITY watchlist validation in the contract crate.

## Task 2 — Event hub

- Track the active Agent command sender, maximum, desired and applied watchlists.
- Validate and atomically dispatch full replacements.
- Preserve idempotence and expose desired/applied revision state.

## Task 3 — Windows Agent

- Read the reviewed sibling argument template once.
- Replace only `--watchlist` for a validated newer revision.
- Terminate/restart the monitor and reconnect with a new generation-bound hello.
- Handle configure commands safely when interleaved with event acknowledgements.

## Task 4 — Integration and documentation

- Document the control-then-subscribe workflow and generated-client fields.
- Update deployment/runtime evidence and gRPC registry wording.
- Add a local bounded probe that never prints credentials.

## Task 5 — Verification

- Run targeted and workspace formatting/tests/Clippy.
- Run compliance, gRPC registry, docs links, and diff checks.
- Build the release binaries, restart the local runtime, replace the watchlist,
  confirm applied status/new generation, and replay events for every requested
  instrument.

All five tasks are complete. The Windows E2E replaced the initial one-instrument
list with `EQUITY:SH:600396,EQUITY:SZ:000001`, applied revision `1`, observed a
new generation, and replayed four observations for each requested instrument.
