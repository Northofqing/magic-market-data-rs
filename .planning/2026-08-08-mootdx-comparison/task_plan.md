# MooTDX comparison analysis

## Goal

Assess the project referenced by the user, compare it with this workspace, and
identify concrete additions without changing product code or architecture.

## Phases

- [x] Identify the linked project and validate its primary upstream sources.
- [x] Inventory the current workspace and `magic-tdx-rs` capability boundary.
- [x] Resolve edge-case differences: extended markets, adjustment, local files,
  caching, CLI/export, and maintenance/licensing risk.
- [x] Deliver a prioritized recommendation with explicit non-recommendations.

## Errors Encountered

- Initial broad repository search output was truncated; replaced it with
  targeted reads of the TDX capability matrix and crate README.
- One `rg` request named obsolete paths `mootdx/adjust.py` and
  `mootdx/cache.py`; the repository instead uses package paths under
  `mootdx/utils`, `mootdx/contrib`, and `mootdx/cache`. The same request still
  returned the target public APIs, and subsequent targeted searches used the
  actual paths.
