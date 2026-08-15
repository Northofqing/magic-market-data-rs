# TDX Dynamic Watchlist gRPC Design

**Status:** Gate A approved by the user on 2026-08-15

## Objective

Allow an authenticated external client to replace the complete TDX local-terminal
monitoring watchlist at runtime. The caller supplies the instruments to monitor;
the repository no longer treats the deployment-file watchlist as the only mutable
control surface.

## Contract

`MarketEventService.SetWatchlist` accepts one non-empty, ordered, duplicate-free
list. Every entry uses the exact canonical form `EQUITY:SH:600000`,
`EQUITY:SZ:000001`, or `EQUITY:BJ:430001`. The operation is full replacement,
not append/remove and not a subscriber-specific union. Repeating the identical
list is idempotent and does not restart the monitor.

The active Windows Agent advertises its positive operator-configured maximum.
The gRPC server rejects a request above that bound before dispatch. A request
when no Agent is active returns `UNAVAILABLE`; it is not persisted as an
unverified future configuration.

The response returns the desired monotonic revision and `restarting` or
`unchanged`. `GetListenerStatus` exposes desired and applied revisions/lists plus
the active maximum. `restarting` is not an assertion that the vendor universe,
loopback health, or first observation has succeeded.

## Agent and continuity

The server sends only a typed `AgentConfigureWatchlist` command over the existing
authenticated TDX Agent stream. It contains a revision and canonical instruments;
it cannot contain a URL, method, executable path, account operation, or threshold.

The Agent validates the command again, terminates the existing fixed sibling
monitor within the configured deadline, replaces only the value paired with the
existing `--watchlist` argument, and starts a new fixed sibling monitor. All
other operator-reviewed arguments remain byte-for-byte unchanged.

Every applied replacement creates a new Agent/terminal generation. Server replay
for the prior generation is cleared when the new hello arrives, and anomaly
windows restart. Events from two watchlists cannot share a generation. The new
Agent hello binds the applied revision, ordered watchlist, and maximum.

## Subscriber semantics

`Subscribe.filter.instruments` remains a delivery filter only. It never changes
the global acquisition set. A controlling client performs:

1. `SetWatchlist` with the complete desired set;
2. poll `GetListenerStatus` until desired and applied revisions/lists match;
3. call `Subscribe` with an optional delivery filter.

This separation prevents one subscriber from silently changing data collected
for another subscriber. Authentication protects both operations; this slice does
not add multi-tenant ownership or per-subscriber monitor processes.

## Failure behavior

- malformed, duplicate, empty, or oversized lists fail before Agent dispatch;
- a closed/full command queue returns an explicit transport error without
  changing desired state;
- stale/non-increasing Agent revisions are rejected;
- monitor restart, TDX discovery, universe validation, loopback, schema, or
  resource failure remains an explicit waiting/unavailable event;
- all LocalTerminal and LocalAnalysis events remain `UNADMITTED`.

No provider admission, TDX path, endpoint, polling threshold, account boundary,
or HTTP registry entry changes in this design.

## Verification

- contract tests cover canonical syntax, duplicates, bounds, and additive wire
  fields;
- EventHub tests cover idempotence, no-Agent failure, command dispatch, and
  desired/applied status;
- Agent tests cover exact argument replacement, revision ordering, and monitor
  generation reset;
- in-process gRPC tests cover `SetWatchlist` followed by status and event flow;
- Windows E2E replaces a one-instrument list with at least two validated equities,
  observes a new generation, and receives events for every requested instrument.
