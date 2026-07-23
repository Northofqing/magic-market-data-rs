# Progress

## 2026-07-23

- Audited the current workspace, provider capabilities, Core contracts,
  packaging and deployment boundaries.
- Built the `ce7f1c6` release package and verified every SHA-256 entry.
- Selected the provider-neutral generic failover-chain design.
- Wrote and self-reviewed the router design; no placeholder or ambiguous
  provider dependency remains.
- Committed the design baseline as `0738d11`.
- Wrote and self-reviewed the exact TDD implementation plan, then moved to
  inline execution under the user's standing no-confirmation instruction.
- Confirmed the Task 1 red test failed only because `SourcedRecord` was absent,
  then added the common evidence trait and eight explicit implementations.
- Recovered from a full data volume by deleting only the 4.8 GiB reproducible
  `target/debug` cache; release artifacts and EMQuant activation files remain.
- Confirmed the Task 2 red test failed because the router package did not exist,
  then added the Core-only crate and explicit source/error abstractions.
- Confirmed the Task 3 red test failed only on absent state-machine types, then
  implemented ordered failover, acceptance gates and complete attempt traces.
- Confirmed the Task 4 red test failed on absent adapters, then added all eight
  provider-neutral Core family adapters without concrete provider dependencies.
- Added the strict real TDX-to-Tencent Quote route. Outside the network-restricted
  sandbox it rejected TDX on missing quality/source evidence, selected Tencent
  at 16.22 with source time `2026-07-23T13:49:34+08:00`, and printed
  `router_live_probe_status=passed`.
- Added routing operations documentation, release packaging integration and a
  compliance assertion that production router dependencies remain provider-neutral.
- After Choice review approval, reran the official EMQuant probe. The SDK still
  returned `10001003` for every supported family with yesterday's activation
  token, so the official activator was opened for a post-approval token refresh.
- Passed the isolated Rust 1.83 full-workspace release preflight.
- Local review corrected Tencent `Core` errors from terminal caller failures to
  retryable protocol failures and added successful-batch passthrough coverage
  for every Core family adapter.
- The first activator launch did not refresh the token and a second SDK probe
  still returned `10001003`; the activator is now running directly from its
  runtime directory so the post-approval SMS refresh can write `userInfo`.
- Passed the final post-review isolated Rust 1.83 release preflight, including
  all workspace tests, strict Clippy, rustdoc/doctests, links and compliance.
- Committed the audited routing integration as `f8ca94e`, generated all five
  release probes from a clean tracked worktree and verified every packaged file
  against `SHA256SUMS`.
- Pushed the completed router release through `bb9450b`; local `HEAD` and
  `origin/main` resolve to the same full commit.
- Confirmed the second SMS activation refreshed the local `userInfo` at
  `2026-07-23 14:22:04 +0800`.
- Reran the complete official Choice probe both while the activator was open
  and after cleanly closing it. Quote, order book, daily money flow, daily bars
  and five-minute bars all stop at SDK login with `10001003/EQERR_NO_ACCESS`.
- Checked the vendor header and example: SDK 2.0+ explicitly permits a null
  login structure for `userInfo` auto-login, and `ForceLogin` only controls
  concurrent sessions. Local token, device, runtime path and process state are
  therefore ruled out; server-side entitlement propagation remains pending.
- Built a temporary login-only official ABI probe and verified
  `start(nullptr, "ForceLogin=1")` outside the network sandbox. It also returned
  `10001003`, conclusively ruling out an old concurrent login as the cause.
- The first release rebuild ran out of disk at the final router compilation.
  Removed only reproducible debug/release caches and the exact partial output,
  preserving historical distributions and the EMQuant runtime. The clean retry
  produced all five probes and every packaged SHA-256 entry passed.
