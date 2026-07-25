# TDX connection-pool close-race design

## Scope

This Gate A repair covers `magic-tdx-rs` blocking connection-pool lifecycle
accounting. It does not change server selection, request retry policy, source
normalization, or any public market-data contract.

## Failure

The heartbeat borrows one connection and may call `close_all` while that guard
is still active. The old implementation reset `active` to zero. Dropping the
guard then subtracted one from zero, panicked, poisoned the pool mutex, and
could abort the process during destructor unwinding.

Connection creation also reserves `active` and `total` before network I/O but
did not release that reservation when connect or handshake failed.

## Data flow and invariants

Each pool generation has an opaque epoch token:

1. pushed, idle, and newly reserved connections capture the current epoch;
2. `close_all` replaces the epoch and closes/removes only idle connections;
3. active counts remain until their guards are returned;
4. a guard from an old epoch is closed and removed, never reinserted;
5. failed connect/handshake reservations are released explicitly;
6. counters use checked transitions and log an invariant failure instead of
   panicking from `Drop`.

At every externally observable boundary:

`total == idle + active`

except during a reserved connection's bounded network construction, where the
reservation is counted as active and is either committed or released.

## Failure modes

- Mutex poisoning remains an explicit process-local failure at existing
  locking boundaries; this repair prevents the known pool lifecycle panic that
  creates the poison.
- Failed connection or handshake returns its original typed error after
  releasing the reservation.
- Returning a stale generation closes the socket and decrements the old
  reservation.
- An impossible counter contradiction is logged and the connection is closed;
  `Drop` never performs unchecked subtraction.

## Old module relation

`ConnectionPool`, `TdxHqClient` heartbeat, and `PooledConnGuard` are retained.
No alternate pool or legacy fallback is introduced. The repair deepens the
existing lifecycle boundary rather than adding a second implementation.

## Validation

- deterministic active-guard/`close_all` regression test
- failed-reservation accounting tests
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features -- --test-threads=1`
- `bash tools/compliance/check.sh`
- workspace coverage thresholds

## Rollback

Revert the BR-029 implementation commit. Until a corrected release is
available, consumers must not run the blocking heartbeat path because the old
pool can abort under the reproduced close/guard race.
