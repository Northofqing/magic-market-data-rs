# TDX Current-Session Admission Design

## Intent

TDX current-minute and current-transaction wire operations may return the
latest cached trading session outside an active A-share session. Raw protocol
access remains available for diagnostics, but normalized provider and gateway
contracts must not relabel those packets as current.

This design is governed by BR-014.

## Data flow

For a `MinuteDataRequest` or `TradesRequest` without a source date:

1. Read the system clock without a default fallback.
2. Convert the instant to Asia/Shanghai weekday and seconds since local
   midnight.
3. Admit the raw current endpoint only during a weekday morning
   `09:30:00..=11:30:00` or afternoon `13:00:00..=15:00:00` session.
4. Outside those windows, return a stable `TdxError::InvalidData` before any
   transport call.
5. Normalize records only after admission succeeds.

Requests with an explicit date continue to use the historical minute or
transaction endpoint and do not consume the wall-clock session gate.

The blocking and async trade paths share the same decision function. Public
request and provider traits are unchanged.

## Failure modes

- Weekend, weekday pre-open, midday break and after-close current requests:
  explicit expected-unavailable error; no raw transport call and no batch.
- System clock before the Unix epoch: explicit error; no default timestamp and
  no transport call.
- Cached raw packets observed by the live probe: printed as
  `diagnostic_unadmitted_*`; never treated as normalized evidence.
- Empty raw current packets: diagnostic only outside an admitted session.
  Inside an admitted session the existing explicit empty-response error
  remains a failure.
- Dated historical request: independent of the current wall-clock session and
  retains the requested source date.

This weekday gate does not claim exchange-holiday awareness. It prevents known
weekend and off-session cache admission; an official trading-calendar gate is
a separate capability.

## Existing module disposition

| Module | Decision | Reason |
| --- | --- | --- |
| `protocol` current operations | retain | Required as bounded raw diagnostics. |
| `net::utils::TradingPhase` | concept reused, implementation not used for admission | Its system-clock path defaults on failure and therefore is not fail-closed evidence. |
| `adapter.rs` normalized minute/trades seams | extend | This is the provider/gateway admission boundary. |
| `live_probe` raw calls | retain as unadmitted diagnostics | They reveal server cache behavior without promoting it. |
| dated history minute/trades | retain unchanged | Explicit source date is independent of current-session admission. |

## Validation

- Deterministic session-boundary tests for weekday morning/afternoon,
  midday, after-close and weekend.
- Blocking minute/trades and async trades prove rejection occurs before raw
  current calls.
- Dated historical minute/trades remain successful outside a current session.
- `cargo fmt`, affected-crate Clippy and tests.
- Bounded real `magic-tdx-rs --example live_probe` ending in
  `live_probe_status=passed`.

## Rollback

Revert this design, BR-014, the adapter gate/tests and probe handling together.
Rollback restores the previous admission behavior but does not delete raw
protocol evidence or change public traits.
