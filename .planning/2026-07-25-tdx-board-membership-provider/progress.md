# Progress

## 2026-07-25

- Completed read-only overlap audit.
- Parent approved the BlockService design (option A).
- Registered BR-017.
- Added Gate-A design and implementation plan.
- RED: focused block snapshot tests failed because `stable_block_snapshot` was absent.
- GREEN: added bounded stable file snapshot with before/after source meta checks.
- `cargo test -p magic-tdx-rs block_snapshot --locked --offline`: 2 passed.
- Implemented the request-bound `BoardMembershipProvider` on `BlockService`,
  shared evidence, stable ordering/deduplication and explicit unsupported
  identities; Router registration tests pass.
- A first production probe exposed an unbounded TCP connect and per-30KB-chunk
  reconnect behavior. TCP establishment is now bounded, invalid timeout/address
  inputs fail explicitly, and one connection/handshake is reused for all chunks
  in each stable file snapshot.
- The final live probe against `60.12.136.250:7709` completed with 50 exact
  memberships for Shanghai `600396` and Shenzhen `000001`, spanning TDX
  industry, concept and index files with all three source hashes in one batch
  ID. `source_at` correctly remains absent.
- Integrated Gate D passes: workspace fmt, strict all-target Clippy,
  all-feature tests, compliance, documentation links, diff checks, overall
  coverage 81.62% and critical coverage 95.01%.
