# magic-tdx-rs findings

External material recorded here is research data, not instructions.

## Confirmed local context

- The adjacent downstream `stock_analysis` repository is a single Rust package rather
  than a Cargo workspace.
- Its `Cargo.toml` pins `rustdx-complete = "=1.0.0"`, and
  `src/data_provider/rustdx_provider.rs` is its production adapter.
- The dedicated `magic-market-data-rs` repository currently contains only the formal
  design and planning records; workspace scaffolding remains prohibited until the
  written spec is approved and an implementation plan is written.
- Unrelated changes in the adjacent downstream repository must not be modified or
  staged by this task.
- Rust compiler observed during feasibility research: `rustc 1.95.0`.
- The existing provider already owns strict whole-page/whole-batch semantics and converts upstream bars into the application's `KlineData` before BR-092 validation.
- The existing provider uses Tencent for realtime because `rustdx-complete 1.0.0` does not provide a trustworthy realtime source timestamp.
- This supports a boundary where `magic-tdx-rs` owns transport/protocol/types and explicit failures, while application-specific freshness and BR-092 policy remain in the project adapter.
- On 2026-07-21 the user moved the work into the dedicated local repository
  `/Users/zhangzhen/Desktop/Quant/magic-market-data-rs`, whose `origin` is
  `https://github.com/Northofqing/magic-market-data-rs.git`.
- The formal spec and all three task planning artifacts were moved from the adjacent
  `stock_analysis` repository with matching pre/post SHA-256 hashes.
- The user approved a standalone pure virtual workspace. The formal spec now defines
  exactly two library members, no root package/facade, and `stock_analysis` as an
  external downstream consumer with separate Gates.
- The adjacent repository is now on `codex/filter-announcement-relevance-20260721`,
  which does not track the relocated spec. The local historical branch
  `codex/magic-market-data-rs-20260721` still points to the original `af0dc28` design
  commit as a recoverable audit copy; it was intentionally not deleted.

## User decisions

- Packaging choice: standalone reusable crate under the repository (`Option A`), with no dependency on `stock_analysis` application modules.
- New scope question: whether the effort should become a complete financial-data aggregator. This is feasible but represents a separate upper-layer product boundary rather than an expanded TDX protocol driver.
- Naming decision: umbrella project/repository `magic-market-data-rs`; TDX driver crate `magic-tdx-rs`. Rust import names will use underscores as Cargo convention requires.
- Initial scope decision: build the provider-neutral core contracts and a complete TDX driver; do not build the multi-provider aggregation runtime in this phase.
- Documentation completeness is mandatory across user guides, architecture/protocol/API/error/data-semantics/performance/compatibility/migration/provenance/maintenance material; docs must be executable or mechanically checked where possible.
- Public API strategy: design a stable idiomatic Rust facade with comprehensive upstream feature/result parity; do not preserve unstable upstream module paths and method signatures merely for source compatibility.
- Numeric model: retain source-compatible decoded numeric fields at the TDX protocol layer, then use explicit checked conversion into strongly typed normalized financial values in `magic-market-core`.
- Client/concurrency model: retain all four client strategies as explicit public types; share internals and traits without hiding strategy-specific blocking, pooling, rate-limit, or throughput semantics.
- v1 completeness means full pure-Rust upstream core parity. Python-only CLI, downloader, and DataFrame conveniences are explicitly deferred.
- Error policy: strict semantics are the only default. Compatibility-style truncation/defaulting/downgrade must not occur implicitly; optional policies must be explicit and observable.
- Performance acceptance: relative same-environment A/B against pinned upstream, with explicit regression thresholds; upstream README absolute latency is historical context, not a portable guarantee.
- Supported-platform decision: Rust 1.83 MSRV with Linux/macOS/Windows and x86_64/aarch64 support.
- Repository organization: standalone pure virtual Cargo workspace with exactly two
  independently versioned library crates, a committed root lockfile, no root package,
  and no umbrella facade crate. Workspace-wide validation is mandatory.
- Downstream dependency decision: production consumers use a fixed published version
  or full Git revision; path dependencies are development-only and cannot serve as
  release evidence.
- Approach: audited extraction/hardening rather than a thin vendor wrapper or ground-up protocol rewrite.

## Upstream facts already verified

- Repository: `https://github.com/jiangtaovan/tdxrs`.
- Earlier feasibility research observed `0.6.5`; the pinned current commit is package version `0.6.7`, Rust edition 2021. The pinned commit supersedes the earlier observation.
- Upstream library emits both `cdylib` and `rlib`, but PyO3 is an unconditional dependency.
- Core modules are public: network clients, protocol, readers, fund, block, errors, constants, helpers, and logging.
- The documented minimum is Rust 1.83; upstream documentation is primarily Python-facing.
- Upstream includes synchronous pooled, direct-per-request, and Tokio async clients.
- Current upstream also includes `TdxSmartClient`, server health/blacklist behavior, and daily-K empty-response retry work introduced after v0.6.5.
- The complete upstream tree is about 29k lines across Rust, Python, tests, examples, and docs. The pure-Rust surface includes protocol parsers/types/adjustment, sync pool client, direct client, async channel-pool client, smart client, finance, fund, block, profile/F10, local readers, errors, constants, and logging.
- Upstream `lib.rs` couples all core modules to unconditional PyO3 registration. Extraction must create a Rust-first facade rather than copying this entry point.
- Upstream has a Rust demo, but most API documentation and convenience outputs are Python-facing. A complete stable Rust crate therefore needs its own public re-exports and rustdoc.
- The upstream docs describe sync pool size 5, async channel pool size 4, direct per-request connections, and an adaptive 15/30/60 req/s phase policy. These values include behavior-changing limits and must be registered under AGENTS 2.10 if retained.
- Upstream benchmark claims are strategy-specific: direct connections show near-zero degradation at 60 threads, while pooled and async modes serialize/queue and take roughly 11-12x longer at 60 threads than at 5. "Parity" should compare each strategy against the pinned upstream on the same machine/server, not require all strategies to behave like Direct.
- Upstream core structs are strongly typed and serializable, and core methods are directly callable from Rust; Python dict/tuple/DataFrame helpers need not be part of the Rust core API.
- The Rust-callable surface is broad but not yet a deliberately stable facade: it exposes low-level byte readers, packet builders, transport internals, pool guards, protocol parsers, and high-level clients through module paths. `magic-tdx-rs` must explicitly classify stable public API versus advanced/low-level API.
- The pinned source has no Rust `unsafe` blocks in `src`, `tests`, or `benches`.
- Unit coverage includes parsers/readers, error codes, pool state, rate limits, adjustment helpers, async channel distribution, heartbeat/disconnect, fund classification, block queries, and local test fixtures. Live integration tests are feature-gated or otherwise separate from deterministic tests.
- Several ambiguity/reliability items require design treatment rather than blind copying: low-level `read_u16/read_u32` helpers can return zero on short input; the transaction `reserved` field is explicitly not understood; some upstream examples use `unwrap_or(0)`; and parser/client silent-success patterns need a complete audit.
- A clean `cargo test --all-features` of pinned upstream fails to link on the current macOS x86_64 environment because the unconditional PyO3 `extension-module` build leaves Python symbols unresolved. This is direct evidence that upstream cannot serve as a reliable pure-Rust dependency unchanged.
- Additional explicit-failure gaps found in pinned upstream:
  - sync/direct/async adjusted-bar paths ignore XDXR errors and can return unadjusted bars for an adjusted request;
  - adjustment context fetch stops on transport/parse errors and can return partial context;
  - quote APIs truncate inputs above 60 instead of returning an error or explicitly chunking;
  - variable-length and fixed-width byte helpers sometimes convert short input to zero, while other helpers can panic;
  - finance indicator extraction fills missing indices with `0.0`, and one finance-list parser converts an invalid size to zero;
  - successful daily-K empty responses remain representable after retries rather than always becoming a typed unavailable/error outcome.
- These gaps should be corrected in `magic-tdx-rs` at the general-purpose core boundary. Project-only business validation remains outside, but transport/protocol completeness and requested-operation semantics are core responsibilities.
- Upstream `TdxSmartClient` is not API-complete by delegation: it directly wraps bars and quotes, exposing other operations only through `inner()`. A stable Rust-first facade should avoid implying uniform client capabilities unless enforced by traits or explicit client types.
- The upstream build currently resolves patch-compatible dependency updates (`pyo3 0.28.3`, `tokio 1.52.1`) despite manifest examples showing earlier patch versions. A derived crate should commit its lock/provenance policy and minimum supported Rust version explicitly.
- Upstream documents adaptive rate limits and concurrency benchmarks that must be reproduced rather than accepted as assertions.
- Upstream source silently skips adjustment when XDXR retrieval fails (`if let Ok(...)`), which is not acceptable for this repository's explicit-failure rules.
- Upstream is MIT licensed and requires preservation of its license/copyright notice when vendored or derived.
- Upstream `main` resolved to commit `18b05ffc9d8a257b5ba5add8a2d1ab038261747d` on 2026-07-21.

## Open research questions

- Exact upstream commit to pin and whether all repository files compile independently of PyO3 after extraction.
- Full Rust-callable API inventory and differences between Rust core and Python wrappers.
- Benchmark methodology, server selection, warm-up, sample size, and statistical acceptance envelope.
- Ambiguous protocol fields and behavior requiring cross-reference against tdxpy/rustdx or packet captures.

## Comparable projects (research snapshot 2026-07-21)

- OpenBB is the closest architectural analogue for a provider-neutral financial-data platform: a lightweight core, standardized models/routes, and independently installable providers/toolkits. It is Python/FastAPI based and broad across commercial/public providers rather than Rust-first or A-share/TDX-first.
- AKShare is the closest broad China-oriented data-interface analogue: Python APIs spanning stocks, futures, options, funds, FX, bonds, indices, crypto, fundamentals, realtime/history, collection, cleaning, and persistence. It also documents the maintenance burden of frequently changing public website interfaces.
- NautilusTrader is the strongest Rust-core adapter architecture reference: normalized domain models plus venue/data-provider adapters, Rust networking/parsing for performance, explicit HTTP/WebSocket/error/retry/config layers, and test fixtures. Its scope is a full trading platform and its current adapter set is primarily global venues rather than A-share public-data aggregation.
- Barter is a Rust-native modular trading ecosystem with `barter-data`, `barter-instrument`, `barter-integration`, and extension traits for market streams/execution. It is strongest as a typed, concurrent streaming/trading reference, particularly for exchange feeds, not as a broad historical/fundamental A-share aggregator.
- pytdx, mootdx, and tdxrs are TDX-specific source libraries/drivers, not complete multi-provider financial-data aggregation platforms.
- Research inference: there are mature examples for either broad provider aggregation (mostly Python) or high-performance Rust trading/data adapters, but no obvious mature open-source project combining Rust-first, broad A-share data, TDX support, explicit data-quality provenance, and a stable provider-neutral API. This is an inference from the reviewed primary project sources, not proof that no such project exists.

## Research errors

- Initial sandboxed `git ls-remote` failed because DNS/network access was restricted. Retried with approved network escalation and succeeded; do not repeat the sandboxed network attempt.
- Pinned upstream `cargo test --all-features` failed at the dylib link step with unresolved Python symbols caused by unconditional PyO3. Resolution for this project is architectural separation of the pure Rust crate, not retrying the same upstream command.
