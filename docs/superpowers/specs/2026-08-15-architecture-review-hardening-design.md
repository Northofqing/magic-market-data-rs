# Architecture review hardening design

## Decision

Apply the review as a bounded correctness and resilience slice without widening
any HTTP Provider contract or changing repository admission. The existing
Core -> Router -> Composition -> Service -> gRPC dependency direction remains
unchanged.

## TDX transport

- Every compressed TCP response is decoded through one bounded helper. The
  declared uncompressed size is an exact contract and the global allocation
  ceiling is 16 MiB.
- The async connection task applies positive connect, read and write deadlines.
  Request waiters also have a deadline, release the pool mutex before awaiting
  a response, preserve round-robin concurrency, retry transport failures after
  rebuilding the pool, and restore chronological pagination order.
- General security queries reject TDX board pseudo-codes before I/O. Connection
  pool and block-client mutex poisoning produces a typed failure or explicit
  recovery rather than a second panic.
- Financial archives use the bounded TDX report protocol only. The unauthenticated
  HTTP port 80 fallback is removed; report names stay inside one fixed `tdxfin`
  component and archive extraction remains size bounded.

## Core and Router contracts

- Every routable record exposes the source/observation evidence it actually
  owns. Record status is checked by Router; stale, conflicted and unsupported
  records are never selected, and callers may require explicit Available state.
- Option contract, quote and Greeks fields are private and can be created only
  through checked input constructors or checked Serde.
- Intraday bars reject cross-date and over-interval spans. Point-labelled bars
  with equal start/end remain supported.
- Eight-digit calendar dates are not interpreted as Unix seconds. Provenance
  always has a non-empty batch ID, and generated IDs include a process-local
  sequence to avoid same-source/same-time collisions.
- ISO date validation is shared by provider request and validated value types.
  Domain-specific minute, offset and evidence-instant parsers remain separate
  because they intentionally accept different wire formats.
- Canonical event digests use the standard `sha2` implementation while retaining
  the existing domain separation, length framing and golden values.

## Service lifecycle and HTTP boundaries

- The TDX Agent sends heartbeats while idle. The server bounds both the first
  hello and every subsequent message wait, disconnects an idle session, and
  prevents commands from accumulating behind a silently dead Agent.
- TLS identity files are read with a 1 MiB ceiling.
- Tencent production traffic is pinned to its fixed quote endpoint. Sina
  production traffic is pinned to its registered hosts and Referers. This is a
  narrowing change and does not widen the HTTP registry.
- Eastmoney public and Miaoxiang clients share one transport in production
  composition, so their combined request stream cannot bypass the one-lane
  pacing policy. Full-market ranking pages are parsed once in production.

## Explicit non-changes

- The two registered HTTP stacks are not merged in this slice. Provider-local
  endpoints are narrowed to fixed registered identities; migrating each legacy
  Provider to `magic-market-transport` changes its TLS/HTTP evidence boundary
  and therefore requires its own Gate A registry update and live revalidation.
- Composition fan-in, exact dependency pins and the generated/checked gRPC
  operation mapping remain deliberate release-engineering trade-offs rather
  than runtime correctness defects. Splitting them is a separate package and
  compatibility migration, not a safe incidental refactor.
- A timed-out `spawn_blocking` Provider call cannot be force-cancelled safely;
  the blocking semaphore remains inside the worker so outstanding work stays
  bounded, while the client-facing unary permit is released at the deadline.
  Cooperative cancellation requires a later Provider-transport design.
- `Price` remains strictly positive. Zero source values mean unavailable or
  ambiguous, not a tradable zero price.
- Strict freshness continues to require record observation time to equal batch
  observation time, as required by BR-033. Providers now construct both from the
  same captured instant.
- The local TDX loopback client remains intentionally single-flight under
  BR-043/BR-048 evidence. Its documented `get_pricevol` list parameter is used
  for one bounded atomic watchlist request, avoiding per-symbol round trips
  without adding parallel calls. Parallel requests and `Arc<str>` record storage
  still require benchmarks and separate contract review.
- Event replay/status cloning remains behind an explicitly bounded EventHub;
  lock sharding is deferred until measurement proves contention at the admitted
  replay limits.
- Eastmoney Miaoxiang diagnostics follow the same explicit request-level
  `allow_unadmitted=true` boundary as every other repository-unadmitted
  operation. A configured Key only makes the fixed diagnostic handler
  available; it does not make that handler default-readable.
