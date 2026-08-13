# Keeyuu/tdx Research Note

Status: deferred research reference; not selected for integration.

Date: 2026-08-12

Upstream: [`Keeyuu/tdx`](https://github.com/Keeyuu/tdx), reviewed at commit
[`9152a955b821f147be0f486f5657801dba0e95f9`](https://github.com/Keeyuu/tdx/commit/9152a955b821f147be0f486f5657801dba0e95f9)
(2022-04-03).

## Decision

Do not integrate this repository into the TDX local-terminal monitor. It is a
research reference for TDX formula/stock-selection plugin mechanics only.

Specifically, the current Gate A work must not:

- copy or translate its Go/C++ implementation;
- add it as a source, binary, DLL, build or runtime dependency;
- load its DLL into the TDX process;
- expose its pattern results as `LocalTerminal` or `LocalAnalysis` events;
- treat its README profitability statement as backtest or production evidence;
- add automated trading, account access or order submission based on it.

Reconsideration requires a separate user decision and separate Gate A work,
including permission/license resolution, clean-room rule specifications,
look-ahead-safe tests, version/ABI compatibility evidence and reproducible
out-of-sample performance evaluation.

## What the upstream implements

The upstream implements two Windows DLL experiments:

1. a TDX calculation-function plugin that passes three `float` arrays through
   C++/cgo to Go functions for single-star, double-star, three-star and fractal
   pattern calculations; and
2. a stock-selection callback plugin that asks TDX for historical bars and
   returns a boolean daily-breakout selection result.

The repository contains no broker account gateway, position management, order
submission, execution reconciliation or complete automated-trading lifecycle.
Its profitability claim is an author statement in the README, not accompanied
by a reproducible backtest, cost/slippage model, drawdown report, trade ledger
or broker reconciliation evidence.

## Material review findings

- The three-star rule calculates `zf2 * 100 / zf2`, which is normally `100`
  and immediately fails its following upper-bound check.
- Fractal detection examines bars after the candidate bar but does not expose a
  separate confirmation timestamp. Historical use therefore risks look-ahead
  bias or repainting.
- The standalone selector passes a returned record count as an array index in a
  path that can read past the final valid item.
- The C++ bridge leaks a heap-allocated `GoSlice`, mismatches `new[]` with
  scalar `delete`, and crosses the FFI boundary with inconsistent output types.
- Tests include unconditional `t.Error` calls and do not establish strategy or
  profitability correctness.
- The Go build targets Windows `386`; the repository does not provide an
  admitted TDX product/version/architecture compatibility matrix.
- No repository license or README license grant was found during review.
  Public visibility alone is not authorization to reproduce or create a
  derivative implementation.

## Relationship to the selected design

The selected work remains the separately designed, read-only local-terminal
path:

```text
running, admitted TDX desktop client
        -> fixed official TQ-Local loopback HTTP endpoint
        -> safe Rust poller and deterministic monitor
        -> typed LocalTerminal observations
        -> optional bounded local or outbound gRPC delivery
```

That path auto-detects an already-running TDX client in the current Windows
user/session, verifies stable process identity, probes only the fixed loopback
origin, starts no poller when the client is absent, and resets monitoring when
the client exits or changes identity. It does not load a vendor DLL, require
Python, install a formula DLL, or modify the TDX installation.
