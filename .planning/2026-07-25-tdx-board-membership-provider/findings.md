# Findings

- Existing Core contract: `BoardMembershipProvider` over `[InstrumentId]`.
- Existing Router adapter: `board_membership_source`; no new production Router code is needed.
- `BlockService` owns blocking `TdxBlockClient`; it creates no Tokio runtime.
- TDX block records expose exact block name/member code but no independent board ID or source
  timestamp. File metadata exposes size and a 32-byte source hash.
- Canonical board code will be `tdx:<filename>:<exact blockname>`.
- `block_fg.dat` is Industry, `block_gn.dat` is Concept, `block_zs.dat` is Unknown because
  Core has no Index category.
- Concurrent dirty overlap exists in adapter/tests/live_probe/router/Core/business rules. Only
  the BR append overlaps; provider work stays in clean block/service files and new files.
