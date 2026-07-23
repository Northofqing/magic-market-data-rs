# Progress

## 2026-07-23

- Restored the completed README plan and ran the session catch-up helper.
- Confirmed that tracked files are clean and preserved the user's untracked
  integration requirements document.
- Created and activated an isolated Sina provider plan.
- Read the complete Tencent reference provider, probes and release/compliance
  integration, then audited the relevant Core contracts.
- Live-verified Sina Quote and K-line responses for Shanghai, Shenzhen and
  Beijing. Confirmed 1/5/15/30/60-minute and daily K lines, current-minute
  derivation inputs, GB18030 Quote encoding and source-share quantity units.
- Confirmed that guessed dedicated minline routes are unavailable and that the
  trade-detail route is presentation HTML, so neither is advertised directly.
- Selected the public Quote + K-line design under the user's standing
  no-confirmation authorization and wrote the design specification.
- The user reported that Choice/EMQuant product entitlement is now enabled.
  Added a final real EMQuant probe after Sina verification so activation and
  entitlement are proved separately.
- Wrote and self-reviewed the exact TDD implementation plan, including
  deterministic/live/load gates, documentation, packaging, delivery and the
  post-Sina EMQuant entitlement check.
- Proved the initial missing-package red state, added the sixth workspace crate
  and passed the public capability/trait compile contract on Rust 1.83.
- Added strict HTTPS transport and GB18030 snapshot parsing with exact
  cardinality/order, redundant top-of-book checks, calendar source time,
  OHLC validation and uniform source-share-to-lot normalization.
- Implemented normalized Sina Quote, five-level OrderBook and partial security
  metadata. Ten deterministic tests pass across Shanghai, Shenzhen and Beijing,
  including limit-up empty asks and malformed/contradictory response cases.
