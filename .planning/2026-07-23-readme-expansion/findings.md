# Findings

- The current root README is only 34 lines and does not explain setup, live
  probes, provider boundaries, routing, packaging or security.
- The workspace contains Core, Router, TDX, Tencent and EMQuant crates and pins
  Rust/Cargo 1.83.0 with `unsafe_code = "forbid"`.
- TDX is the broad pure-Rust source. It live-verifies quotes, 12 K-line
  categories, books, minute data, trades, securities, finance, actions, blocks,
  funds and F10, but not normalized money flow or auction.
- Tencent is a supplemental public-web source with live-verified quotes, books,
  selected K lines, minute data, current Shanghai/Shenzhen trades and partial
  metadata; it has no SLA and does not provide the missing business families.
- EMQuant code maps quotes, bars, minute bars, order books and daily money flow,
  but the current account still fails SDK login with `10001003`; auction,
  trades and metadata remain unsupported or unverified.
- The router is provider-neutral, preserves attempt evidence and selects the
  first batch that passes explicit completeness/source-time policy. It does not
  cache, merge providers, schedule work or provide a daemon/API.
- Release packaging produces five diagnostic binaries and a SHA-256 manifest
  from a clean tracked worktree. Vendor SDK files and activation tokens are
  deliberately excluded.
- EMQuant implements minute intervals through `HistoricalBars`; it does not
  implement the separate normalized `MinuteData` trait. The README must keep
  minute K lines and intraday minute-point data distinct.
- Most normalized records carry `observed_at`, while `Bar` relies on batch
  provenance `fetched_at`; the common evidence description must name both.
