# TDX 本地终端行情监听技术设计

Status: Gate A transport selected and implemented as a diagnostic slice. Data
family admission remains independent and false until its evidence is complete.

Date: 2026-08-12
Route: B — optional Windows leaf service, library-first workspace preserved

## 1. Decision

The implementation uses the official TQ-Local HTTP interface exposed by a
running TDX client:

```text
POST http://127.0.0.1:17709/
```

Rust calls this loopback endpoint directly. The production design does not use
Python, `tqcenter.py`, a vendor DLL, an undocumented native ABI, injected code,
screen scraping or UI automation. The earlier vendor-DLL proposal is retired
and is not a fallback.

The official documentation states that the TDX client must be running before
the endpoint is used and documents the `{id, method, params}` envelope. The
version notes also state that TQ data can be called without Python and identify
TQ-Local support.

References:

- <https://help.tdx.com.cn/quant/docs/markdown/mindoc-1hdhbmi50d038.html>
- <https://help.tdx.com.cn/quant/docs/markdown/mindoc-1cfsjkbf8f3is/TdxQuantVersion.html>

## 2. User-visible behavior

The operator does not configure the TDX installation path, a DLL path, Python,
or the TQ endpoint.

1. The optional monitor executable starts.
2. A short-lived Windows discovery helper searches the current interactive
   session for an exact `TdxW.exe` owned by the current user.
3. If no client exists, the service remains in `WaitingForTerminal`. It does not
   poll TQ-Local or expose a caller-facing event listener.
4. If exactly one client exists, the service records executable identity and
   performs a bounded health/schema request to the fixed loopback endpoint.
5. A compatible response performs one bounded `get_stock_list` read for the
   vendor-defined all-A-share universe and requires every explicit watchlist
   identity to be an exact member before any price/volume/snapshot request.
   Missing, duplicate or malformed universe identities fail closed; the
   service never accepts a six-digit code alone as equity proof.
6. Only a completely validated watchlist starts polling.
7. Client exit, response failure, sequence discontinuity or incompatible schema
   resets the affected monitor windows and returns the service to a typed
   waiting/backoff state.
8. Client restart creates a new stream generation. Events never silently join
   observations from different generations.

Multiple matching clients are an explicit ambiguous state. The service does not
guess which client owns port 17709.

## 3. Architecture

```text
running TdxW.exe
    |
    | owns official loopback endpoint
    v
http://127.0.0.1:17709/
    |
    | bounded blocking POST, read-only method enum
    v
magic-tdx-local-rs                (safe Rust transport/contracts)
    |
    | typed observations and lifecycle status
    v
magic-market-monitor              (pure deterministic rules/replay)
    |
    v
magic-market-monitor-server       (optional Windows leaf executable)

magic-tdx-native-bridge --discover
    short-lived Windows process discovery only; no data access or DLL loading
```

Existing provider, router and composition behavior is unchanged. Core and
router do not depend on the optional service. The server is a leaf composition,
not a replacement for the library-first deployment model.

## 4. Component boundaries

### 4.1 `magic-tdx-native-bridge`

Despite the historical package name, its only admitted role is a short-lived
Windows discovery helper. It:

- enumerates exact `TdxW.exe` processes;
- verifies current session and current user;
- reports bounded executable identity and provenance;
- distinguishes absent, unique and ambiguous clients;
- never loads or inspects a vendor data DLL;
- never sends market-data requests;
- never exposes a long-lived `--serve` data path.

The sole workspace `unsafe` exception is limited to audited Windows discovery
calls in this binary. All market-data and monitoring crates remain safe Rust.

### 4.2 `magic-tdx-local-rs`

This safe crate owns:

- the exact loopback origin;
- a closed, read-only TQ method enum;
- request/response schemas and decimal validation;
- request ID and instrument matching;
- explicit connect/read/write timeouts;
- request and response byte limits;
- proxy-disabled and redirect-disabled HTTP behavior;
- typed transport, HTTP, RPC and schema errors;
- source sequence validation and lifecycle state machines;
- separation of repository admission from runtime availability.

It does not accept an endpoint override. Adding a method requires Gate A review,
an allowlist change, schemas, tests and admission evidence.

### 4.3 `magic-market-monitor`

This crate is transport-neutral and deterministic. It receives observations,
cursors and explicit reset signals; it does not read the wall clock, discover
processes or perform I/O.

Price, cumulative amount and cumulative volume are independent families. A
missing amount must not block price or volume monitoring and must never be
replaced by zero. Each family has independent rules, admission and runtime
availability.

Rules have explicit IDs, revisions, canonical definition digests, window
parameters, trigger/rearm thresholds and cooldowns. There are no production
default window sizes or thresholds.

### 4.4 `magic-market-monitor-server`

The optional binary composes discovery, TQ polling and monitor rules. It owns:

- an explicit equity watchlist whose entries carry asset, exchange and exact
  code (`EQUITY:SH|SZ|BJ:dddddd`), plus explicit polling/rule limits;
- the blocking-worker boundary required by `async-blocking.md`;
- discovery/health/poll/backoff lifecycle;
- bounded event output and shutdown;
- generation changes and reset injection.

TDX path, discovery-helper path and endpoint are deliberately not configurable;
the server resolves the helper beside its own executable and uses the fixed
loopback origin. Watchlist, cadence, transport bounds, rule parameters,
identity-recheck cadence, restart budget, diagnostic cycle bound, maximum event
bytes, output queue capacity, output shutdown timeout and slow-consumer policy
are all explicit and have no production defaults. The implemented slice emits bounded four-byte
big-endian length-prefixed JSON frames to stdout; it does not emit JSON Lines.
An inbound WebSocket/HTTP service is a later, separately registered transport
and must remain absent until admitted.

Fast `get_pricevol` price/volume polling remains serialized in the scheduler.
The independently paced `get_market_snapshot` amount family uses one bounded
worker with capacity one. A busy worker produces typed backpressure instead of
blocking or replaying fast-family observations; snapshot failure resets only
the amount window.

Before either data method is used for a new terminal generation, the scheduler
serially calls the official read-only `get_stock_list` method with exact
`market="5"` and `list_type=0`. The bounded response must contain a non-empty,
duplicate-free list of canonical `dddddd.SH|SZ|BJ` identities, and every
configured `EQUITY` watchlist member must be present. This identity gate is
generation-scoped and is repeated after terminal replacement. It is not a
security-metadata data family and does not promote any admission boolean.

Stdout serialization uses a bounded non-blocking producer queue and one writer
thread. The only admitted slow-consumer policy spelling is `stop`: queue full,
writer failure or bounded shutdown timeout stops the diagnostic explicitly. It
does not drop frames, block the polling scheduler or claim a delivery guarantee.

## 5. HTTP contract and safety policy

The only origin is `http://127.0.0.1:17709/` and the only path is `/`.

- Environment proxies are disabled.
- Redirects are disabled, including redirects to another loopback port.
- Connect, read and write timeouts are explicit positive inputs.
- Request and response bodies are bounded before unbounded allocation.
- Calls are serialized unless a later evidence-backed concurrency policy is
  approved.
- Response ID must equal request ID.
- Content type, JSON structure, RPC `ErrorId`, requested instrument and decimal
  fields are validated.
- Account, position, order, cancel and other trading methods are prohibited.
- Raw vendor JSON is not retained or emitted by default.

The closed read-only method set is `get_stock_list`, `get_pricevol` and
`get_market_snapshot`. `get_stock_list` is used only for the generation-scoped
equity-identity gate. `get_pricevol` maps only:

| TQ field | Internal meaning | Current state |
| --- | --- | --- |
| `Now` | price in CNY/share | diagnostic observation implemented |
| `Volume` | cumulative volume in lots (`手`) | diagnostic observation implemented; no implicit share conversion |
| `LastClose` | response/schema validation evidence | not yet a monitor input |
| amount | unavailable from this method | remains `None`; never zero-filled |
| source timestamp | unavailable | remains `None` |
| source record count | unavailable | remains `None` |

`get_market_snapshot` is implemented and has a checked-in captured response.
The 2026-08-13 request for exact instrument `600396.SH` used the minimal
`field_list` `[Amount, Now, Volume, LastClose]` and returned
`Amount="127354.65"`, `Now="17.62"`, `Volume="735536"` and
`LastClose="17.18"`. The installed vendor sample
defines total `Amount` as ten-thousand CNY and other snapshot quantities at
their ordinary displayed unit; a cold bounded one-symbol call returned in about
11 seconds, while a later captured call completed in under one second. This
spread is not a production latency budget. It is therefore a separate paced
family, not part of the fast price/volume polling cadence. Rust maps the
captured `Amount` to `1273546500` CNY only by exact checked decimal
multiplication by 10,000. Volume retains the official lot (`手`) unit. The
response supplies neither source timestamp nor source record count. `ItemNum`
must not be renamed to transaction count or
source-record count without official evidence.

## 6. Version compatibility

Version awareness must not turn into manual configuration.

- Discovery records the exact executable identity available from the running
  client (path, architecture, digest and version fields when obtainable).
- A known executable identity may be reported as known evidence.
- An unknown or updated executable is not rejected solely because its digest is
  new. The fixed-origin HTTP health/schema probe decides whether the implemented
  read contract is compatible.
- Schema drift, missing fields, changed units, RPC errors or an unavailable
  endpoint fail closed with typed status.
- Every event carries stream/source provenance sufficient to identify the
  generation that produced it.
- Repository production admission remains per data family and cannot be
  promoted by a successful runtime probe alone.

This policy makes an installed compatible TDX client directly usable while
still detecting version changes and refusing unknown semantics.

## 7. Lifecycle

The lifecycle is explicit:

```text
Disabled
  -> Discovering
  -> WaitingForTerminal             (none)
  -> AmbiguousTerminal              (>1)
  -> ProbingLoopback                (one)
  -> Running                        (schema compatible)
  -> BackingOff                     (bounded failure)
  -> WaitingForTerminal             (client exit)
  -> CircuitOpen                    (restart budget exhausted)
  -> Stopping -> Stopped
```

No infinite tight retry loop is allowed. Retry budgets and durations are
explicit runtime inputs until shadow evidence supports production values.
Shutdown is bounded and children are always waited so no orphan process remains.

## 8. Observation and event contracts

Each observation has:

- exact instrument;
- provider identity `LocalTerminal`;
- caller-observed UTC time;
- monotonic arrival value supplied by the lifecycle owner;
- optional family values, never fabricated;
- optional source time and record count only when proved;
- stream generation/sequence cursor;
- observation and source continuity.

Each anomaly event has:

- rule ID, revision and canonical definition digest;
- instrument and exact first/last input evidence;
- time basis and both continuity dimensions;
- ordered cursors from one generation;
- deterministic input digest and event ID;
- derived provider identity `LocalAnalysis`;
- typed transition and output cursor.

Replay identity is independent of emission time. Deserialization recomputes
canonical digests and rejects tampering.

## 9. Independent data-family admission

These repository admissions remain separate:

- terminal price;
- terminal cumulative volume;
- terminal cumulative amount;
- terminal source record count;
- local price anomaly;
- local volume anomaly;
- local amount anomaly.

Runtime capability/entitlement can only deny availability; it cannot promote a
repository admission. Production construction retains an independent
`ImplementationUnavailable` gate until the real end-to-end composition exists,
so changing booleans alone cannot create a false production success.

## 10. Calendar and reset semantics

The monitor never guesses trading sessions from weekdays or the host clock.
Trading-date changes, session boundaries and midday breaks arrive as explicit
reset signals from an approved calendar/session owner. Until that integration
exists, rules that require session semantics remain unadmitted.

Terminal exit, stream-generation change, sequence gap, cumulative-family
rollback and explicit calendar/session changes reset only the affected windows.

## 11. Performance and sizing

No replay, queue, poll, restart or client limit is selected by guesswork.

Local diagnostic evidence on 2026-08-13:

- `get_match_stkinfo` returned a valid response in about 0.6 seconds;
- ten successive `get_pricevol` calls returned HTTP 200 in about 19–51 ms;
- the Rust diagnostic probe subsequently observed about 12 ms latency;
- one cold snapshot call completed in about 11 seconds and a later captured
  call completed in under one second, so it must not block the fast
  price/volume loop and no cadence may be inferred yet;
- the historical `get_market_data` path did not complete inside one 10-second
  diagnostic attempt and therefore has no admitted latency budget.
- a later complete server diagnostic ran 12 configured cycles and exited zero;
  all three monitors warmed, snapshot/fast price-volume cross-checks passed,
  every admission remained false and the snapshot worker joined at shutdown.

These observations are evidence, not production defaults. Shadow operation must
record latency percentiles, response sizes, failure bursts, terminal restarts,
watchlist cardinality and event frequency before choosing defaults.

## 12. Failure and provenance rules

Failures are preserved as typed states; they are not converted to empty market
data or success:

- terminal absent or ambiguous;
- loopback connect/timeout/transport failure;
- redirect, unexpected status or content type;
- request/response size violation;
- invalid JSON, RPC error or response-ID mismatch;
- unknown/missing instrument or invalid decimal;
- sequence gap, rollback or exhaustion;
- cumulative rollback;
- incompatible schema/version;
- unsupported platform;
- restart budget exhausted.

`source_at` remains absent if TQ does not supply and define it. `observed_at`
must never be copied into `source_at`.

## 13. Security and account boundary

This integration is read-only market monitoring. Source code must not declare,
construct or dispatch account/trading method names. The loopback endpoint is
still a trust boundary: origin/path are fixed, proxy and redirect behavior is
disabled, inputs and outputs are bounded, and logs redact raw payloads.

The optional event service is not remotely exposed. Remote binding, TLS,
webhooks, durable brokers, raw retention and licensed-data redistribution are
outside this design.

## 14. Testing strategy

### Pure and fixture tests

- exact request JSON and method allowlist;
- fixed host/path and proxy/redirect rejection;
- connect/read/write timeout and body limits;
- malformed content type/JSON/RPC/id/instrument/decimal;
- discovery none/one/multiple/current-session/current-user cases;
- price-only and volume-only observations with amount absent;
- sequence duplicate/gap/rollback/exhaustion;
- terminal loss/restart/backoff/circuit/shutdown;
- deterministic price/amount/volume rules and explicit resets;
- event digest, replay and Serde tamper resistance;
- source time/count absence is preserved.

### Windows live tests

- no TDX client: waiting, no poller/listener;
- running supported TDX client: automatic discovery and health success;
- unknown/upgraded executable with compatible schema: identity recorded, health
  decides compatibility;
- endpoint unavailable/schema drift: typed failure and no production event;
- client exit/restart and multiple clients;
- watchlist limits, latency, response size and resource stability;
- read-only source scan proving no trading methods.

### Repository gates

Before release run formatting, workspace tests, Clippy, docs/link checks,
compliance checkers, dependency policy, audit/deny checks and Windows-specific
jobs. The full `docs/data-sources-inventory.md` validation remains a separate
acceptance matrix: provider probes and downstream application E2E status must
not be conflated.

## 15. Gates

### Gate A — design and registries

- official loopback transport and exact policy recorded;
- business rule and HTTP transport registry synchronized;
- read-only method allowlist reviewed;
- automatic discovery/version policy reviewed;
- no-DLL/no-Python decision explicit.

### Gate B — deterministic foundation

- safe HTTP contract and fake endpoint tests;
- discovery helper and lifecycle state machine;
- family-selective observations;
- deterministic monitor/event/replay contracts;
- all production admission booleans remain false.

### Gate C — bounded live and shadow evidence

- Windows lifecycle/live tests;
- field units and source semantics proved per family;
- approved calendar/session owner;
- measured sizing and restart policy;
- no account/trading surface;
- per-family registry evidence updated.

### Gate D — optional production service

- real end-to-end composition replaces `ImplementationUnavailable` atomically;
- only proved families are enabled;
- production packaging/CI/platform claims updated after admission; the current
  Windows diagnostic pair does not satisfy this Gate D item;
- caller-facing inbound transport, if any, receives its own Gate A and registry
  row before implementation.

## 16. Current implementation checkpoint

Implemented:

- provider-neutral event identity/evidence/replay foundations;
- deterministic price, amount and volume rule foundations;
- safe fixed-origin TQ-Local `get_stock_list`, `get_pricevol` and
  `get_market_snapshot` clients, with complete A-share watchlist validation
  before the first market observation;
- bounded HTTP/error/schema tests and Rust live diagnostic probe;
- Windows current-user/session TDX discovery foundation;
- numeric executable file/product version, version-source/failure provenance
  and explicit periodic process-identity recheck;
- transport-neutral supervisor and fake peer tests;
- optional Windows diagnostic server lifecycle with explicit equity watchlist,
  fixed sibling discovery, typed failure/reset output and diagnostic cycle
  bound;
- independently paced capacity-one amount worker and family-selective reset;
- bounded four-byte big-endian length-prefixed JSON stdout events;
- bounded stdout writer queue with fail-closed slow-consumer and shutdown
  policies;
- Windows-host diagnostic packaging of the server/helper pair, both still
  `admitted=false`;
- one bounded 12-cycle Windows end-to-end run with framed output, three-family
  warm-up and joined snapshot-worker shutdown;
- all data-family admission constants false by default.

Still required before production admission:

- bounded Windows lifecycle runs still covering absent/ambiguous/restart/exit,
  plus sustained operation of the packaged pair beyond the completed 12-cycle
  diagnostic;
- official evidence for source time/source-record count semantics;
- calendar/session integration;
- sustained shadow measurements and version-upgrade tests;
- caller transport Gate A, if a network listener is desired;
- full workspace and inventory validation report.
