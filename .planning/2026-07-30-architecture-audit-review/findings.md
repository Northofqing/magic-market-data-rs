# Findings & Decisions

## Requirements
- Assess whether the listed issues are factually accurate and materially important.
- Identify recommendations that would be risky, breaking, or mis-prioritized.
- Give a practical sequence consistent with repository Gates A-D and registered business rules.
- Do not modify product code.

## Research Findings
- The working tree was clean before this review; only this review's isolated `.planning` files and active-plan pointer are now uncommitted.
- Workspace has 31 members: core/router/composition/transport/analysis plus provider crates.
- Manifest evidence confirms a split HTTP stack: 14 crates declare pinned `ureq = "=2.12.1"`; 10 provider crates depend only on `magic-market-transport`; `magic-exchange-rs` declares both. TDX has its own socket stack.
- `magic-eastmoney-rs` directly declares `ring = "=0.17.14"`; `magic-market-transport` declares `rustls` with the `ring` provider. This does not by itself mean two compiled copies of `ring`; Cargo normally unifies identical semver-compatible versions. `cargo tree -d` must decide that claim.
- Internal `magic-market-transport` path dependencies are pinned with `version = "=0.2.0"`.
- `deny.toml` currently has `multiple-versions = "warn"` and `wildcards = "allow"`.
- Workspace already has `unsafe_code = "forbid"` and Clippy `all = deny` centrally.
- `AGENTS.md` is intentionally terse and `docs/ENGINEERING_RULES.md` currently contains only the four one-line gates, so the user's description of rich Gate documentation is overstated for the current tree.
- Source evidence strongly confirms repeated local transport machinery in the ureq providers: local agents, timeout checks, zero redirects, response-size/MIME/final-URL validation, and local error mapping.
- `cargo tree -i ring` proves the resolved workspace has one `ring v0.17.14`, shared by Eastmoney and rustls/reqwest/ureq. The claim that providers carry an “independent ring” is false at the resolved graph level even though they expose separate direct manifest dependencies/HTTP policy implementations.
- `magic-exchange-rs` is itself hybrid: it depends on the shared transport and ureq, while several exchange modules use the local ureq transport.
- The claim that almost no provider supports deterministic transport injection is false in the current tree. Many ureq providers publish provider-specific transport traits and public `with_transport` constructors (Eastmoney, Tencent, Sina, Baidu, CLS, Jin10, ThePaper, Gov, Yonhap, WallstreetCN, THS, CNInfo, iWencai, and exchange clients). Shared-transport providers also expose injection seams.
- However, the injection types are fragmented (`HttpTransport`, `EastmoneyTransport`, `SnapshotTransport`, etc.), so the real issue is lack of a common policy/protocol contract—not lack of testability.
- A trait-only `magic-market-transport` is unnecessary as a first step because it already exposes `HttpTransport` and a concrete `ReqwestTransport`. A safer migration is to preserve the trait and add an alternative backend/adapter only if provider-specific needs can be represented without weakening strict endpoint policies.
- Router is intentionally synchronous and ordered. `FailoverChain::route` is serial, so cumulative timeout latency is real.
- Concurrent “first success” is not a drop-in performance enhancement: it changes ordered failover semantics, may hit every upstream unnecessarily, complicates terminal `Stop` handling and attempt ordering, and cannot cancel already-running blocking ureq/reqwest calls. It needs a separate policy/API and admission/load-budget design, not merely `std::thread::scope`.
- The router already requires each adapter constructor to accept a caller-supplied `classify: Fn(Provider::Error) -> SourceError`; the manual classification burden is deliberate and located at the composition edge. A blanket `From<TransportError>` would lose provider/action context, while putting router `FailureKind` into core provider traits would invert dependencies.
- `RouteAttempt` and `AttemptStatus` already derive `PartialEq + Eq`; the audit's retry/dedup claim is stale. They also expose stable typed `ProviderId`, `FailureKind`, and `FailureAction`, so a message hash is not needed for normal policy decisions.
- Async core traits are not all dead code: `AsyncTdxHqClient` implements `AsyncHistoricalBars`, `AsyncRealtimeQuotes`, and `AsyncTrades`. Only `AsyncMinuteData` appears unimplemented. The statement that TDX is not attached to core async traits is false.
- `ProviderId` lacks `#[non_exhaustive]`, but current repository searches show very few production exhaustive matches. Adding it is a semver-breaking downstream change, not a zero-cost annotation. It can be reasonable before 0.2.0 release, but needs a migration note and wildcard policy.
- TDX unsupported MoneyFlow and Auction contracts are enforced in code via trait implementations returning typed `TdxError::Unsupported`; they are not “documentation/tests only”. This is a major stale claim in the audit.
- The six named TDX network client files total about 5,680 lines, not ~8,300. The broader point about a large API/implementation surface remains valid.
- `PoolConfig` is public and `ConnectionPool` accepts it, but the synchronous `TdxHqClient` hard-codes a default config in `new()` and does not expose a corresponding constructor. The requested configuration seam is valid. The async client already has `with_pool_size`.
- `ConnectionPool` does serialize short bookkeeping operations under one mutex, but deliberately releases the mutex before TCP connect/handshake I/O. Calling this a proven hotspot requires benchmark/profile evidence; mutex existence alone is insufficient.
- TDX exposes more than four public high-level clients/services (`TdxHqClient`, async/direct/smart/finance/fund/profile/block plus services). API discoverability and ownership boundaries deserve review, but collapsing them blindly to sync/async may erase genuinely distinct protocols.
- The synchronous TDX contention issue is stronger than the audit states: `try_send_and_recv` holds `MutexGuard<Arc<ConnectionPool>>` while the borrowed connection performs send/receive, because the pool guard's lifetime backs the connection guard. This outer handle mutex can serialize the entire synchronous request path and defeat pool concurrency. Cloning the `Arc` while briefly holding the outer mutex would release that global handle lock before `borrow`.
- Composition truly has one concrete binding, but its docs explicitly call it “the first binding” and its non-forgeable constructor is a specialized security/admission property. Generalizing it into an ordinary provider factory could weaken the reason it exists. Better to add bindings only where the same non-forgeability requirement exists and document that criterion.
- README is 933 lines, not 2,500+. It is still dense, and there is no `GETTING_STARTED.md`/provider-matrix split, so the onboarding concern is directionally valid but numerically outdated.
- “Documentation examples are zero” is false: README and `docs/MULTI_PROVIDER_ROUTING.md` contain Rust examples, and at least `docs/integrations/level2-auction.md` has one. Crate-level rustdoc examples are sparse, so the better finding is poor discoverability/doctest coverage, not absence.
- Adding `rust-toolchain.toml` directly contradicts an approved Gate A design and an active compliance rule that intentionally rejects such a file. `channel = "stable"` also does not make builds reproducible; it is rolling selection. This recommendation must not be implemented without a new architecture decision replacing the 2026-07-23 unpinned-toolchain design.
- README's opening is indeed dominated by a long date-stamped status block before the product boundary and usage. The product-writing recommendation to lead with the positive value proposition and move volatile provider status to generated/reference docs is strong.
- README already explicitly tells production applications to supply concurrency budgets, rate limiting, circuit breaking, caching, persistence, trading-phase freshness, and monitoring. Thus lack of built-in tracing is an extension-point opportunity, not an unacknowledged operational promise.
- The audit's “no CI/check catches `*_ADMITTED` drift” claim is false. `tools/compliance/check_admissions.py` discovers every tracked public `*_ADMITTED: bool`, requires a registry row, and compares the Rust boolean to the TSV; compliance runs it.
- Admission counts/dates/blockers are still human-entered. Automation could improve provenance, but parsing probe stdout and overwriting TSV during preflight would be brittle and could erase reviewed evidence. Prefer structured, signed probe artifacts plus a deterministic `render/check --diff` command; keep the committed registry review-gated.
- Release preflight is intentionally offline and does not run live probes. Therefore it cannot safely generate current live admission counts itself; live evidence collection and offline release verification must remain distinct stages.
- The blocking-runtime warning is genuinely missing from public docs. Shared transport uses `reqwest::blocking`; local providers use ureq; `RequestGate::wait_for_turn` sleeps a thread. A prominent “blocking API; use `spawn_blocking` in async services” guide is a high-value, low-risk fix.
- `RequestGate` already releases its reservation mutex before sleeping, as documented. The issue is thread blocking, not lock-held sleep.
- Exact `=0.2.0` internal path constraints are workspace-wide, not limited to composition/router. Removing `=` broadens published compatibility to the usual caret range, but it does not specifically fix `cargo update -p serde`; that causal explanation is wrong. Treat this as a release/versioning policy decision.
- `cargo tree -d` currently reports two `webpki-roots` versions (0.26.11 and 1.0.9), arising from the split TLS/HTTP ecosystem. Flipping `multiple-versions` from warn to deny now would fail unless the transport graph is unified or an explicit skip policy is added. It should be a later ratchet, not an independent P1 change.
- Core's domain modules are private (`mod capital`, etc.); users cannot currently import `magic_market_core::capital::...`. The audit's suggested path would first require making modules public and intentionally adding a second stable API surface.
- Reordering `pub use` items does not itself break downstream Rust paths. A curated `prelude` can improve ergonomics but does not remove or shrink the existing stable root exports. This is documentation/API-governance work, not a correctness problem.
- The claim that non-TDX crates broadly use `println!/eprintln!` in production source is unsupported by repository search. Direct stderr logging is concentrated in TDX's custom logging macros; the broader transport layer has no observer/tracing hook.
- Most provider client `new()` constructors already return `Result`. `TdxHqClient::new()` only initializes local state and an empty pool; it does not connect. Forcing it to return `Result` would add ceremony without surfacing network failures earlier. Keep fallible connect explicit; add fallible configuration constructors only where local validation can fail.
- The shared body reader's `take(max + 1)` is correct bounded-stream detection. HTTP chunk boundaries do not create a false extra byte; a `(max + 1)`st decoded body byte means the response truly exceeds the limit. It already stops after that byte.
- Initial 64 KiB capacity can cause geometric reallocations for larger allowed bodies. A safe micro-optimization is to use a validated/capped `Content-Length` hint, but `BytesMut` is not required and untrusted lengths must never drive uncapped allocation. This is low priority and benchmark-dependent.
- The router's two `find` scans are real, but linear scans over in-memory records are likely dwarfed by network/parse work. A single validation loop can preserve typed first-failure semantics, yet should be benchmarked before prioritization.
- There are no conventional Cargo bench targets in the workspace. `cargo bench --workspace` would not create the desired continuous TDX baseline. The repository already has a custom controlled release-profile harness; any CI history should extend that deterministic workload/artifact model rather than add an empty/noisy generic bench command.
- The approved TDX Gate A design explicitly requires Blocking, Async, Direct, and Smart as four first-class strategies. Collapsing to two public clients would contradict the design. The better remediation is to finish the promised builders/configuration and improve selection guidance, then deprecate only demonstrably redundant facades via a new decision.
- BR-003 promises five blocking connections and four async connections. The outer synchronous pool-handle lock spanning I/O is therefore not merely a hypothetical hotspot; it conflicts with the intended pooled concurrency semantics and deserves high priority.
- No synchronous TDX multi-request pool concurrency test was found; the async client has a four-connection concurrency test. Add a deterministic loopback regression proving more than one synchronous request can be in flight before claiming the pool fix.
- `CONTRIBUTING.md` is also terse and does not contain detailed Gate A-D descriptions. Worse, it says preflight uses a “pinned minimum toolchain”, contradicting the approved/current rolling-stable policy. The audit missed this concrete documentation drift.

## Technical Decisions
| Decision | Rationale |
|----------|-----------|
| Use repository evidence rather than accept line references at face value | The audit text has duplicated/misaligned passages and may target an earlier tree state |
| Keep TDX TCP distinct from HTTP transport consolidation | It is a different wire protocol; the debt is duplicated HTTP policy, not the existence of protocol-specific networking |
| Preserve ordered Router semantics as the default | Racing providers changes load, terminal-failure, priority, cancellation, and provenance semantics |
| Do not couple Core provider traits to Router `FailureKind` | That would reverse the intended dependency boundary; classification belongs in router adapters/composition |
| Treat `ProviderId #[non_exhaustive]` as a pre-release SemVer decision | Valuable future-proofing, but breaking for downstream exhaustive matches |
| Preserve four TDX strategies until a new Gate A decision | The current approved design assigns distinct semantics to Blocking/Async/Direct/Smart |

## Recommended Order
1. **Immediate correctness/performance:** release the TDX outer pool-handle lock before network I/O and add a deterministic synchronous concurrency regression test.
2. **Immediate release documentation:** document all HTTP providers as blocking, show Tokio `spawn_blocking`, correct the contradictory pinned-toolchain text, and shorten/move volatile README status.
3. **Gate A transport design:** freeze new one-off HTTP policy implementations; define shared conformance requirements and a migration matrix for the 14 ureq crates, including endpoint-specific MIME/size/TLS needs.
4. **Pre-0.2 API decisions:** decide `ProviderId #[non_exhaustive]`; resolve the lone unimplemented `AsyncMinuteData` promise; expose validated TDX configuration/builders without making infallible constructors fallible.
5. **Phased transport migration:** move providers behind the shared policy/observer/error taxonomy one family at a time with deterministic and live evidence. Ratchet duplicate-dependency policy only after the graph permits it.
6. **Separate hedged-routing design:** add an opt-in async/hedged strategy only with explicit deadline, priority, concurrency/load budget, cancellation, and attempt-order semantics. Do not implement it as scoped blocking threads.
7. **Later ergonomics/measurement:** structured admission artifacts, curated prelude/namespaces, body allocation and batch-scan benchmarks, dedicated performance history.

## Issues Encountered
| Issue | Resolution |
|-------|------------|
| The submitted audit repeats several sections and mixes numbering | Treat repeated text as one claim set and verify current repository paths |

## Verification
- `python3 tools/compliance/check_admissions.py`: PASS, 17 capabilities.
- `cargo tree -i ring`: one resolved `ring v0.17.14`.
- `cargo tree -d`: only `webpki-roots` is duplicated among the inspected TLS/HTTP packages.
- `git diff --check`: PASS.

## Resources
-
