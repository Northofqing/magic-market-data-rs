# TDX connection-pool close-race design

## Scope

This Gate A repair covers `magic-tdx-rs` blocking connection-pool lifecycle
accounting and the internal transport seam needed to verify connection
configuration without assuming that a test runner may bind a loopback socket.
It does not change server selection, request retry policy, source
normalization, or any public market-data contract.

## Failure

The heartbeat borrows one connection and may call `close_all` while that guard
is still active. The old implementation reset `active` to zero. Dropping the
guard then subtracted one from zero, panicked, poisoned the pool mutex, and
could abort the process during destructor unwinding.

Connection creation also reserves `active` and `total` before network I/O but
did not release that reservation when connect or handshake failed.

The external connection regression also bound `127.0.0.1:0` to prove timeout
configuration and byte I/O. Coverage runners may correctly deny listener
creation, causing the test fixture to fail before it reaches production code.
That environmental assumption is not part of the `TcpConnection` interface.

## Data flow and invariants

Each pool generation has an opaque epoch token:

1. pushed, idle, and newly reserved connections capture the current epoch;
2. `close_all` replaces the epoch and closes/removes only idle connections;
3. active counts remain until their guards are returned;
4. a guard from an old epoch is closed and removed, never reinserted;
5. failed connect/handshake reservations are released explicitly;
6. counters use checked transitions and log an invariant failure instead of
   panicking from `Drop`.

`TcpConnection` keeps its existing public interface. Internally, one private
stream seam owns connect, timeout configuration, byte I/O, peer-state and
shutdown operations. The production adapter remains `TcpStream`; deterministic
tests use a memory adapter that exercises the same connection implementation
without opening a listener. A private connector seam returns either that
production stream or an injected transport error, so connection-error mapping
and timeout bounds remain covered.

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
- Invalid address and timeout values continue to fail before invoking the
  connector.
- Connector, timeout-configuration, send and receive failures remain typed
  `TdxError::Connection` values; an early EOF remains
  `TdxError::Disconnected`.

## Old module relation

`ConnectionPool`, `TdxHqClient` heartbeat, and `PooledConnGuard` are retained.
No alternate pool or legacy fallback is introduced. The repair deepens the
existing lifecycle boundary rather than adding a second implementation.
`TcpConnection::connect/send/recv/close/is_open` are also retained unchanged;
the memory adapter is private test support and cannot enter a production data
path.

## Validation

- deterministic no-socket generation/accounting regression for an active
  return after `close_all`
- failed-reservation accounting tests
- deterministic no-listener connection error, timeout, send/receive, EOF and
  shutdown tests through the unchanged connection implementation
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features -- --test-threads=1`
- `bash tools/compliance/check.sh`
- workspace coverage thresholds

## Rollback

Revert the BR-029 implementation commit. Until a corrected release is
available, consumers must not run the blocking heartbeat path because the old
pool can abort under the reproduced close/guard race.
