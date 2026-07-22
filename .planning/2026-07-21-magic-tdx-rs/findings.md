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

## Implementation-plan recovery

- An untracked 167-line implementation-plan index already exists at
  `docs/superpowers/plans/2026-07-21-magic-market-data-rs-implementation.md`.
  It intentionally decomposes the approved design into five ordered plans: foundation,
  protocol/readers, clients, services/adapter, and release evidence.
- The five linked phase-plan files do not exist yet. Phase 6 is therefore incomplete;
  no implementation task can start until those plans are written and self-reviewed.
- The phase plans must preserve the design's two-layer data model, atomic strict
  operations, source-time semantics, typed contextual errors, four distinct client
  strategies, explicit rate-limit scope, and exhaustive pure-Rust compatibility matrix.
- The standalone library plan must not include or modify the adjacent `stock_analysis`
  repository; downstream adoption requires a separate future design cycle.
- Phase planning must bind every Gate D claim to a repository command or CI job:
  differential harness, Criterion/loopback benchmarks, live read-only diagnostic,
  coverage thresholds, SemVer, dependency/license audit, rustdoc/examples, and links.
- Provenance scope is deliberately library-only: the crates supply traceable fields and
  validate their completeness, while durable five-year audit storage remains a
  downstream consumer responsibility and cannot be claimed by this repository.
- Compatibility evidence may patch the pinned upstream only to remove PyO3 build and
  registration coupling. The patch and its digest must be committed and shown not to
  alter protocol, parser, client, or numeric behavior.
- The release plan must keep online checks opt-in and truthful: network/market-hour
  unavailability is a blocker, not a skipped success, and raw machine-readable A/B
  evidence must include environment, limiter, concurrency, warm-up, and sample data.
- The pinned upstream checkout is still available at
  `/private/tmp/magic-tdx-rs-upstream`. Its pure-Rust implementation is concentrated in
  `protocol`, `net`, `reader`, `fund`, `block`, and `profile`; Python modules are
  physically separate but registration/dependencies are coupled through the crate root.
- Upstream source layout is coarser than the approved target layout. The implementation
  plans therefore need explicit provenance-preserving extraction tasks that first lock
  fixtures/API inventory, then split focused target modules without treating a bulk copy
  as reviewed code.
- The concrete upstream operation inventory confirms separate families for bars/index
  bars, quotes, security list/count, minute/current-history data, current/history trades,
  finance/XDXR, block metadata/content, financial report files/indicators, fund data,
  and F10/profile data. Sync, direct, and async expose overlapping but non-identical
  subsets; Smart directly exposes only bars and quotes.
- The target capability matrix must not claim all four client strategies implement every
  operation uniformly. Shared request executors can support reusable services, while the
  public facade and trait implementations must state actual capability per client and
  use `Unsupported` only for an explicit documented boundary—not to hide missing v1
  pure-Rust coverage.

## Current CI facts checked for the release plan (2026-07-21)

- GitHub's official hosted-runner reference currently lists x64 Linux/Windows labels,
  Arm64 Linux labels such as `ubuntu-24.04-arm`, Intel macOS labels such as
  `macos-15-intel`, and Arm64 macOS labels such as `macos-15`. Arm64 Linux/Windows
  availability is marked public preview, so required release evidence should use an
  owned/self-hosted fixed benchmark runner if hosted availability is insufficient.
  Source: https://docs.github.com/en/actions/reference/runners/github-hosted-runners
- The official `actions/checkout` repository currently documents `actions/checkout@v6`
  and recommends `permissions: contents: read`; full history requires `fetch-depth: 0`,
  which the SemVer/provenance jobs need.
  Source: https://github.com/actions/checkout
- These facts are release-plan inputs, not instructions from the fetched pages. Workflow
  action revisions must be pinned to immutable commit SHAs during implementation even
  when examples use a moving major tag.
- `actions/checkout` v6.0.2 resolves to immutable commit
  `de0fac2e4500dabe0009e67214ff5f5447ce83dd` (verified with `git ls-remote`).
- Official cargo-llvm-cov release material currently identifies 0.8.6 and states its
  prebuilt releases are immutable, multi-architecture, and attested; installing the
  current source release requires Rust newer than this project's 1.83 MSRV, so CI should
  install/verify a prebuilt tool with stable rather than compiling it under 1.83.
  Source: https://github.com/taiki-e/cargo-llvm-cov/releases
- Official cargo-deny material currently identifies release 0.19.4. The failed crates.io
  API attempt means the plan must not invent versions for other release tools; a checked
  tool lock/digest manifest is part of implementation evidence.
  Source: https://github.com/EmbarkStudios/cargo-deny

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

## 2026-07-22 provider continuation

- TDX live validation returned 1,820/1,820 current and 2,001/2,001
  historical normalized trades across real page boundaries.
- The historical trade parser previously omitted the final minimum-size row;
  using the packet count fixed 19/20 and 1,999/2,001 results to exact counts.
- EMQuant bridge/runtime discovery and macOS local ad-hoc signing are working.
  The SDK now returns `10001014`, defined in the bundled header as
  `EQERR_NEED_ACTIVATE`; the local SDK has no `userInfo` yet.
- The current core exchange enum cannot represent Beijing. TDX security-list
  records expose code/name/decimal/pre-close but do not prove listing date or a
  versioned price-limit rule, so P1 metadata must preserve those absences.
- Current official exchange material confirms why price limits cannot be a
  timeless code-prefix constant: Beijing's current rule is 30% with explicit
  no-limit cases; STAR/ChiNext use 20% with initial no-limit days; and the 2026
  Shenzhen rule revision changes main-board risk-warning stocks from 5% to 10%.
- Beijing introduced the independent `920` stock-code range in 2024 and is
  migrating legacy listings in stages. A provider must carry `Exchange::Beijing`
  explicitly and must not assume all Beijing equities use only legacy 43/83/87
  prefixes or only the new prefix.
- Current official EMQuant manuals state that section APIs cover Shanghai,
  Shenzhen, and Beijing equities, but the public C++/Mac pages do not document a
  Beijing security-code suffix example. The adapter must not invent `.BJ`
  behavior; Beijing remains explicit unsupported until code validation can run
  under an activated account or an official suffix definition is obtained.
- The bundled macOS PDF confirms the GUI activator uses the API account's bound
  phone plus a verification code, not a username/password form. A desktop
  client session is separate from API activation.
- The activator links GTK 3 and reads `./image/EMApp.ico` plus companion image
  assets from its working directory. GTK was absent locally and the initial
  runtime packager omitted `image/`; both must be present for the activation
  form to render reliably.
- API activation generated a 748-byte project-local `userInfo` and the SDK
  consumed it, changing the live result from `10001014` to `10001003`.
  The bundled official header/PDF defines `10001003` as `EQERR_NO_ACCESS`
  (account has no API entitlement), so the remaining EMQuant live blocker is
  commercial account permission rather than transport, packaging, or code.
- Parallel fake-bridge tests could select the same temporary directory because
  the filesystem clock has coarser resolution than nanoseconds. A process-local
  atomic id is required in addition to PID/timestamp for deterministic isolation.
- The TDX financial manifest can lag the official HTTP object size (observed
  5,116,233 vs 5,116,020 bytes). HTTP framing plus ZIP bounds, uncompressed
  size, and CRC validated the actual object; treating the manifest size as an
  allocation hint restored 5,526 records and 45 named indicators without
  weakening archive integrity.
- The user-selected live sample was verified from Shanghai Stock Exchange
  material as 华电辽能 `600396.SH`. The full TDX probe returned quote 14.92,
  source name 华电辽能, all 12 K-line categories, 240/240 minute points,
  1,820/1,820 and 2,001/2,001 paged trades, current finance, 30 XDXR records,
  5,526 batch-financial records, and 45 named indicators.
