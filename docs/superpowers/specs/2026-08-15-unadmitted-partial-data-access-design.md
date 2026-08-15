# Explicit diagnostic partial-data access

## Outcome

Expose source-backed fields that the repository can already retrieve without
promoting an unadmitted provider capability. Missing fields remain absent in the
canonical record, and diagnostic responses are always marked `UNADMITTED` and
`complete=false`.

This is an additive Gate A change to the external gRPC boundary. It does not add
or widen an HTTP endpoint, transport, redirect policy, timeout or response-body
limit. Every diagnostic handler reuses an already registered provider-local
transport and endpoint allowlist.

## Contract

`QueryRequest` gains the backward-compatible field:

```proto
bool allow_unadmitted = 4;
```

The default is `false`. With the default, repository-unadmitted operations fail
before Provider I/O exactly as before. With `true`, a caller may execute only a
repository-registered diagnostic handler. The server response then has:

- `admission = UNADMITTED`;
- `complete = false`, even if the diagnostic source batch happens to be strict;
- `diagnostic_blocker` containing the repository admission blocker;
- normal provider, batch, record-schema, provenance and source-time fields.

An unadmitted registration without a diagnostic handler still fails before I/O.
The switch cannot select arbitrary providers, URLs, methods or endpoints.

## Initial diagnostic handlers

| Operation | Provider | Data returned | Boundary retained |
| --- | --- | --- | --- |
| `TechnicalBars` | Baidu | source OHLCV/amount and optional MA5/10/20 | calendar and corporate-action continuity remain unproved |
| `FundFlowSeries` | Eastmoney | source-supplied main/size-bucket values; unavailable fields remain `null` | stability/load admission remains false |
| `MoneyFlows` | Eastmoney | latest point from a bounded one-row instrument fund-flow request | never maps TDX cumulative turnover into money flow |
| `FuturesDelivery` | CFFEX | diagnostic calendar records, including `NotProvided` method | TLS/live admission remains false |
| `PostCloseFlows` | Eastmoney | bounded diagnostic records for an explicit current or past date and per-record source evidence | every row must match the requested date; mixed source times remain incomplete and non-atomic |
| `MarketRankings` | Eastmoney | first bounded source page, provider-reported universe size, and nullable row fields | no complete-market coverage or atomicity claim |

`Auctions` remains without a diagnostic handler because ordinary quotes do not
prove the BR-035 auction semantics. `MarketBreadth` remains without a handler
because an incomplete universe cannot produce the BR-034 aggregate honestly.
Callers can use the already admitted `RealtimeQuotes` and
`ProviderTopNRankings` operations for those available, differently named facts.

## Failure and security behavior

- Authentication, mTLS, message bounds and concurrency are identical to
  admitted reads. The Provider HTTP timeout remains an explicit bounded value
  no greater than 60 seconds, while the blocking-worker deadline is configured
  separately so a complete multi-page diagnostic can finish without widening
  any individual HTTP timeout.
- Provider errors remain typed gRPC failures; they do not become empty success.
- A diagnostic handler can never set repository admission or runtime admission.
- Callers must display or isolate diagnostic results and must not use them for
  production alerts or trading decisions.
- Secrets and provider credentials remain process environment inputs and never
  enter the request payload, response, blocker or logs.

## Verification

- Core service tests prove default fail-before-I/O and explicit opt-in behavior.
- gRPC contract tests prove old clients decode the new field as `false`.
- gRPC server tests prove response admission and blocker propagation.
- Composition tests cover every diagnostic registration and preserve the two
  operations that cannot be represented honestly.
- Provider fixture/live probes continue to own field and endpoint evidence.
