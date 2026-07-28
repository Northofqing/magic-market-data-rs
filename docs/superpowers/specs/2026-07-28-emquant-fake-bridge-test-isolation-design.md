# EMQuant Fake-Bridge Test Isolation

**Date:** 2026-07-28
**Status:** Approved for implementation
**Business rule:** BR-036

## 1. Decision

The Unix EMQuant integration tests execute synthetic shell bridges only inside
test-owned temporary directories. Each fixture owns one unique directory and
one immutable published executable for its complete lifetime. Tests may run in
parallel, but they must never overwrite an executable path after publication
or rely on a shared process-global bridge file.

This rule changes test infrastructure only. The production
`EmQuantClient` command, timeout, normalization, provenance and financial-data
semantics remain unchanged.

## 2. Fixture lifecycle

```text
unique test directory
  -> create-new staging file
  -> write complete script
  -> sync and close writer
  -> set executable permissions
  -> atomic rename to never-before-published final path
  -> construct client from borrowed fixture path
  -> execute and wait/kill through the public client contract
  -> drop client
  -> drop fixture guard and remove its unique directory
```

Timeout and malformed-response variants receive their final script at fixture
construction. They do not mutate the default executable in place.

## 3. Failure modes

| Condition | Required behavior |
| --- | --- |
| Directory or staging-file creation fails | fail the test explicitly |
| Staging write, sync, chmod or publish fails | fail before client construction |
| Final executable path already exists | fail; never truncate it |
| Concurrent fixtures resolve to one path | regression test fails |
| Bridge spawn/timeout/normalization fails | retain the public typed-error assertion |
| Fixture guard drops | remove only its own unique test directory |

## 4. Old-module relation

| Module | Decision | Reason |
| --- | --- | --- |
| `EmQuantClient::execute` | retain unchanged | production command and timeout semantics are not the defect |
| timestamp/process/counter directory naming | retain and harden | already reduces collision risk; RAII and create-new publication complete the contract |
| in-place `fs::write` overrides | delete | an executable path must be immutable after publication |
| bare `PathBuf` fixture return | replace | it does not express directory ownership or cleanup lifetime |

## 5. Verification and rollback

The integration suite must prove several fixtures can be created, retained and
published simultaneously with distinct executable paths and complete contents.
Existing public-client tests exercise each published bridge, and the complete
test binary is then repeated under the default parallel harness before
workspace gates.

Run:

```bash
cargo fmt --all -- --check
cargo test -p magic-emquant-rs --test fake_bridge
cargo test -p magic-emquant-rs --all-targets
cargo clippy -p magic-emquant-rs --all-targets -- -D warnings
bash tools/release/preflight.sh
```

Rollback only this test-infrastructure slice with `git revert <commit>`. Do
not alter production EMQuant behavior to make a synthetic fixture pass.
