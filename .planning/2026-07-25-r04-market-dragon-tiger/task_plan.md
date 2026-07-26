# R-04 Market Dragon-Tiger

## Goal

Deliver a real-source, atomic whole-market dragon-tiger discovery and seat
detail contract for downstream R-04.

## Phases

- [x] Gate A pre-flight and source-contract discovery
- [x] Gate A design and BR registration
- [x] Core RED/GREEN
- [x] Eastmoney discovery RED/GREEN
- [x] Eastmoney seat RED/GREEN
- [x] Router RED/GREEN
- [x] Live probe
- [x] Gate C/D workspace validation and handoff

## Constraints

- Preserve unrelated dirty work.
- No mock data in production paths.

## Errors

| Error | Resolution |
| --- | --- |
| Required repo instruction files beyond `AGENTS.md` are absent | Recorded; followed repository `AGENTS.md` and parent repository hard rules |
| Workspace `cargo fmt --all -- --check` initially reported concurrently edited Sina files | Integrated all shared slices, formatted the workspace, and reran the full release gate successfully. |
