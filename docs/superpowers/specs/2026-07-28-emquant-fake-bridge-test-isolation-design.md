# EMQuant Fake-Bridge Test Isolation

**Date:** 2026-07-28
**Status:** Approved for implementation
**Business rule:** BR-037

## 1. Decision

The Unix EMQuant integration tests execute only checked-in, executable shell
fixtures from `crates/magic-emquant-rs/tests/fixtures/`. The test process never
creates, writes, chmods, renames or deletes a pathname that it later executes.
Tests may run one immutable fixture concurrently because no test owns a write
capability to the executable inode.

This rule changes test infrastructure only. The production
`EmQuantClient` command, timeout, normalization, provenance and financial-data
semantics remain unchanged.

## 2. Fixture lifecycle

```text
Git checkout provides mode 100755 fixture
  -> resolve fixture below CARGO_MANIFEST_DIR
  -> verify regular-file and executable mode
  -> construct client from fixture path
  -> execute and wait/kill through the public client contract
  -> drop client without mutating fixture
```

Default, timeout and malformed-response variants are separate checked-in
executables. No variant mutates another executable in place.

## 3. Failure modes

| Condition | Required behavior |
| --- | --- |
| Fixture is absent or not a regular file | fail before client construction |
| Git executable mode is absent | fail before client construction |
| Test attempts a runtime fixture write | no write API exists in the test module |
| Concurrent calls execute one fixture | every public-client call must succeed |
| Bridge spawn/timeout/normalization fails | retain the public typed-error assertion |
| Test completes | leave the checked-in fixture unchanged |

## 4. Old-module relation

| Module | Decision | Reason |
| --- | --- | --- |
| `EmQuantClient::execute` | retain unchanged | production command and timeout semantics are not the defect |
| runtime staging/rename fixture publication | delete | Linux CI still exposed `ETXTBSY` at spawn despite closing the writer; a test does not need runtime executable publication |
| checked-in executable fixtures | adopt | removes every test-process write capability to executable inodes |
| bare `PathBuf` fixture return | retain for checked-in paths | ownership and cleanup are unnecessary because the test never mutates the file |

## 5. Verification and rollback

The integration suite must prove one checked-in immutable bridge can be
executed simultaneously by several public clients. Existing public-client
tests exercise every fixture variant, and the complete test binary is then
repeated under the default parallel harness before workspace gates.

Run:

```bash
cargo fmt --all -- --check
cargo test -p magic-emquant-rs --test fake_bridge
cargo test -p magic-emquant-rs --all-targets
cargo clippy -p magic-emquant-rs --all-targets -- -D warnings
bash tools/release/preflight.sh
```

Rollback only this test-fixture slice with `git revert <commit>`. Do not alter
production EMQuant behavior to make a synthetic fixture pass.
