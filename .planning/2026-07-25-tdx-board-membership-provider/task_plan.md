# Task plan: TDX board-membership provider

## Goal

Implement BR-017 in the isolated upstream release worktree without altering Core contracts,
adding BoardCatalog, or overwriting concurrent TDX/R04/Sina work.

## Phases

- [x] Phase 1: Read instructions, audit dirty overlaps, inspect contracts and source facts.
- [x] Phase 2: Present alternatives, obtain approval, write Gate-A design/BR/implementation plan.
- [x] Phase 3: RED/GREEN stable block-file snapshot.
- [x] Phase 4: RED/GREEN request-bound BoardMembershipProvider and failures.
- [x] Phase 5: Router registration tests, bounded real probe, docs.
- [x] Phase 6: fmt/tests/strict Clippy/Rustdoc/docs/compliance/diff gates.
- [x] Phase 7: Integrated release handoff.

## Decisions

- Implement the existing trait on `BlockService`.
- No Core or Router production contract changes.
- `source_at` remains absent; observed time is local only.
- Complete three-file no-match is a complete empty batch.
- One connection and handshake is reused for every chunk in one block-file
  snapshot; metadata is still checked before and after the full download.

## Errors encountered

| Error | Attempt | Resolution |
|---|---:|---|
| Gate-A combined patch context missed concurrent BR-016 edits | 1 | Re-read the tail and applied a narrower append context. |
| Initial live probe could wait for the operating-system connect timeout | 1 | Replaced unbounded `TcpStream::connect` with validated `connect_timeout` and added deterministic connection failure tests. |
| Per-chunk direct connections multiplied timeout/handshake cost across large block files | 1 | Reused one bounded connection per file for metadata, every 30KB chunk, and final metadata; the live probe then completed successfully. |
