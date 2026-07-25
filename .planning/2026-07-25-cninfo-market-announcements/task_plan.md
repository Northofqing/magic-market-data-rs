# Task plan: CNInfo whole-market announcements

## Goal

Implement BR-018 as a real Core/Provider/Router capability without changing
downstream `stock_analysis` or disguising per-instrument queries.

## Phases

- [x] Phase 0 — Read rules, inspect existing contracts, live-check native
  market protocol, obtain design approval.
- [x] Phase 1 — Register BR-018 and write Gate A design/implementation plan.
- [x] Phase 2 — TDD Core request and Provider contract.
- [x] Phase 3 — TDD CNInfo native market pagination and evidence.
- [x] Phase 4 — TDD Router adapter and verified-empty policy.
- [x] Phase 5 — Add and run bounded real probe.
- [x] Phase 6 — Run focused tests, scoped fmt, strict Clippy and diff checks.
- [x] Phase 7 — Review scoped diff and report exact release status.

## Decisions

- Use CNInfo's native market list with `stock=` empty.
- Preserve source order; apply limit only after complete-page validation and
  stable deduplication.
- Map exchange only from exact source `pageColumn`.
- Accept a complete empty route only through a default-off policy.

## Errors

| Error | Attempt | Resolution |
| --- | --- | --- |
| Missing downstream engineering files in this upstream worktree | 1 | Applied the upstream root `AGENTS.md`; no downstream dependency introduced. |
| CNInfo Shanghai plate probe returned HTTP 504 | 1 | Did not retry the same call; whole-market, Shenzhen and Beijing probes already proved native protocol and identity fields. |
| First production probe rejected `totalpages=36,total=1108,pageSize=30` | 1 | Recorded that CNInfo reports the floor quotient, preserved it as source evidence, and independently derived the actual ceil page count and `hasMore` boundary. The corrected probe admitted a complete batch. |
