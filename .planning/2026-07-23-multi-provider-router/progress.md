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
