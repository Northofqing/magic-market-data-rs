# gRPC External API and TDX Listener Design

**Status:** Gate A approved in conversation on 2026-08-13

Caller integration guide: `docs/integrations/grpc-external-api.md`.

## 1. Objective

Expose the repository's provider-neutral read-only capabilities to controlled
external systems through one versioned gRPC boundary. The same boundary carries
TDX local-terminal observations and anomaly events without exposing the vendor
loopback endpoint, provider transports, account functions, or downstream path
dependencies.

gRPC is the primary protocol. HTTP/JSON and gRPC-Web are optional future
gateways and are not part of this slice.

## 2. Non-negotiable boundaries

- Core, Router and Provider crates never depend on gRPC, Protobuf or the server.
- External callers select a closed operation and validated request data; they
  never supply a URL, transport, proxy, TLS policy, provider client or arbitrary
  executable path.
- Repository admission and runtime availability remain independent. A network
  method cannot promote an unadmitted capability. Unadmitted operations fail
  before provider I/O with a typed gRPC status and structured error details.
- Provider records retain provider identity, batch identity, completeness,
  source evidence, source time when supplied, local observation time and units.
- The API never fabricates source time, source cardinality or an empty-success
  batch from a failure.
- Account, cash, position, order, cancel and execution operations are outside
  the schema and source tree, preserving BR-035.
- Current blocking Provider clients execute only in an explicitly bounded
  blocking worker boundary. gRPC/Tokio workers never call them directly.

## 3. Workspace architecture

```text
external Rust/Go/Java/Python/C# clients
                  |
             gRPC HTTP/2
                  v
      magic-market-grpc-server       binary-only leaf
                  |
      magic-market-grpc-contracts    generated wire contract
                  |
         magic-market-service        blocking use-case facade
                  |
       composition / router / core
                  |
              providers

TdxW.exe -> fixed 127.0.0.1:17709 -> Windows monitor process
         -> magic-market-tdx-agent -> authenticated client stream
         -> bounded event hub -> subscriber server streams
```

`magic-market-grpc-contracts` contains only generated Protobuf types and wire
validation helpers. `magic-market-service` owns the exhaustive operation
registry, typed application errors and production composition. The server owns
transport policy, authentication, blocking isolation, health/reflection and
event fan-out. `magic-market-tdx-agent` is a Windows leaf that supervises the
existing monitor process, validates its four-byte big-endian JSON frames and
publishes them to the server; it does not interpret vendor DLLs or expose an
inbound socket.

## 4. API services

### 4.1 SystemService

- `GetCapabilities`: complete closed capability inventory including repository
  admission, runtime availability, exact supported scope and blocker.
- `GetHealth`: process liveness and dependency readiness without secrets.

### 4.2 MarketDataService

The contract has a distinct RPC for every current provider-neutral read family:

- historical bars, minute data, realtime quotes, money flows, order books,
  auctions, trades and security metadata;
- global indices, foreign exchange, economic calendar, futures delivery,
  reference rates, official FX fixings, economic series and company filings;
- global/instrument news, announcements, market announcements, investor
  questions and policy documents;
- security profiles, financial statements, statistics, technical bars and
  corporate actions;
- board directory, constituents and memberships;
- research reports/documents, consensus, target price and semantic search;
- fund/board/post-close flows, margin, block trades, holder counts, lockups,
  dividends and northbound daily statistics;
- limit pools, strong-stock reasons, dragon-tiger data/discovery, market
  rankings/breadth, popularity, concept hits, option data and Provider Top-N.

Each method uses a method-specific operation identity and a versioned canonical
payload envelope. Phase one carries canonical UTF-8 JSON records inside the
Protobuf envelope so existing Serde contracts remain the single normalization
source. The envelope binds a non-empty `record_schema`, positive
`record_schema_version`, content type, request ID and bounded bytes. Replacing a
canonical payload with strongly typed Protobuf records is a compatible per-family
migration only after golden cross-language evidence; it must not silently change
the current Rust serialization contract.

### 4.3 MarketEventService

- `Subscribe`: server streaming from a requested current generation/cursor.
- `Replay`: bounded same-generation best-effort replay.
- `GetListenerStatus`: TDX discovery, health, generation and admissions.
- `SetWatchlist`: authenticated full replacement of the global canonical equity
  monitoring set; desired/applied revisions remain distinct until Agent restart.

Each event binds generation, checked sequence, event kind, provider, instrument,
rule identity/version where applicable, observation/source continuity, source
and observation time, units, admission and the original bounded monitor frame.
Slow subscribers receive `RESOURCE_EXHAUSTED` with the last delivered cursor;
events are never silently dropped or described as at-least-once.

### 4.4 TdxAgentService

- `OpenStream`: authenticated bidirectional stream. The first message is a bounded
  agent hello; subsequent messages carry ordered status/observation/anomaly
  events. Server messages contain lifecycle acknowledgement, an explicit
  stop/reconnect instruction, or a versioned bounded watchlist replacement,
  never URLs, thresholds, account or trading commands.

The agent connects outward. The central server never connects to TQ-Local and
never exposes port 17709. Agent identity changes, terminal generation changes,
sequence gaps, stream loss and failed schema validation reset affected windows
and are represented explicitly.

## 5. Wire compatibility

- Protobuf package: `magic.market.v1`.
- Every request contains a non-empty caller-generated request ID and protocol
  version 1. Unknown enum values are rejected, not mapped to a default family.
- New optional fields may be added compatibly. Field numbers are never reused;
  removed fields are reserved.
- Every bounded byte/string/repeated field is checked again at the application
  boundary; Protobuf decoding success is not business validation.
- gRPC reflection is disabled unless explicitly configured because it expands
  externally visible schema discovery. Health is always a separate allowlisted
  service.

## 6. Error mapping

Application errors map deterministically:

| Application error | gRPC code |
| --- | --- |
| invalid request/schema/cursor | `INVALID_ARGUMENT` |
| capability unadmitted/unsupported | `UNIMPLEMENTED` |
| missing or invalid credentials | `UNAUTHENTICATED` |
| authenticated but not entitled | `PERMISSION_DENIED` |
| concurrency/stream queue exhausted | `RESOURCE_EXHAUSTED` |
| provider deadline/worker deadline | `DEADLINE_EXCEEDED` |
| provider temporarily unavailable | `UNAVAILABLE` |
| source protocol/completeness violation | `FAILED_PRECONDITION` |
| internal invariant violation | `INTERNAL` |

The response status message contains no secret, response body, API key, SEC
identity or arbitrary upstream URL. Structured safe details include request ID,
operation, provider when selected, retryability and evidence-safe reason code.

## 7. Runtime and resource policy

- Default bind is exact loopback. A non-loopback bind is accepted only with an
  explicit remote-bind switch, TLS identity and authentication policy.
- Maximum decode/encode bytes, unary concurrency, blocking concurrency, stream
  subscribers, per-subscriber queue, replay count/bytes and graceful-shutdown
  timeout are positive required operator inputs. No proposal number becomes a
  production default.
- Unary requests acquire a service semaphore before `spawn_blocking`. Provider
  socket timeouts remain authoritative because dropping an async future does not
  cancel an already-running blocking call.
- TDX agent ingress and subscriber fan-out use separate bounded queues. A slow
  subscriber cannot block TDX polling or another subscriber.
- CORS is irrelevant to native gRPC and no browser origin is enabled.

## 8. Authentication and TLS

Loopback-only diagnostic use may run with a required bearer token loaded from a
secret environment/file boundary. Remote bind requires TLS and either mTLS or a
separately approved signed-token verifier. Credentials are never accepted in a
query payload, logged or returned by capability/health methods.

The TDX agent runs as the same Windows user as TDX but authenticates to the
central service as an agent identity. One agent identity has one active terminal
stream unless an explicit replacement handshake closes the old generation.

## 9. Admission and release state

Creating an RPC does not admit its data family. `admissions.tsv` remains the
authority. FRED, SEC, exact NBS/PBC/World Bank scopes and other admitted rows may
be production-composed. IMF, CFETS DR007, all current LocalTerminal families and
all LocalAnalysis anomaly families remain fail-closed until their existing
evidence blockers are independently resolved.

TDX gRPC streaming is therefore initially diagnostic and emits
`admitted=false`. It must not be relabeled production merely because transport,
streaming and replay tests pass.

## 10. Verification

- Protobuf descriptor golden test and Buf-style breaking compatibility check.
- Cross-language encode/decode fixtures for Rust plus at least one generated
  non-Rust client.
- Unit tests for every operation's admission-before-I/O behavior and error map.
- In-process gRPC tests for size, auth, deadline, concurrency and shutdown.
- Stream tests for duplicate/rollback/gap, slow consumer, replay unavailable,
  terminal replacement and reconnect.
- Windows E2E: TDX absent, present, exit, restart and fixed-loopback failure;
  emitted events remain diagnostic until admission changes separately.
- Workspace format, tests, Clippy, docs, transport/compliance checks and release
  packaging run before Gate D.

## 11. Packaging

The cross-platform package may contain the gRPC server. Windows packages also
contain the TDX agent, monitor server and discovery helper in one signed/hash
manifest. Libraries and default constructors never start either listener or
agent. Service startup is an explicit operator action.
