# Findings & Decisions

## Requirements
- Fix all confirmed issues from the audit, not the disproved claims.
- Preserve explicit failures, source evidence, Gates A-D, and one-way
  dependencies.
- Do not rewrite the checked `f64` value layer merely for exact equality.
- Do not add release optimizations without measured evidence.

## Research Findings
- TDX `read_u16`/`read_u32` and `get_price` silently return zero on
  truncation. Multiple production parsers accept partial declared batches.
- TDX tests currently accept a truncated security list as an empty success;
  the fuzz smoke test checks only absence of panics.
- The low-level TDX helper modules are public, so changing signatures is an
  explicit pre-1.0 API hardening that must be documented.
- Core value constructors and custom deserializers already reject NaN and
  infinity.
- Manual epoch-plus-eight-hour formatting is independent of host timezone,
  but the calendar conversion is duplicated across several Providers.
- Core post-close validation already parses and bounds the full clock, so the
  reported BR-019 bypass is not reachable through normalized records.
- The root manifest has no release profile; the claimed 10-30% benefit has no
  repository benchmark evidence.
- Exchange's local gate intentionally holds one mutex across pacing and full
  I/O. Its TLS backend choice and injected transport seam must survive any
  migration.
- Exchange is not the only Provider outside `magic-market-transport`; only the
  newer official/global Provider group consistently uses it.
- Current admitted PBC, CFETS, Xinhua, Yicai, and STCN families have documented
  live/load evidence. NBS, FRED, IMF, World Bank, and SEC remain explicitly
  unadmitted.
- Router production source is 3,074 lines, with 8,932 lines of tests. Its
  ordered first-acceptable-source behavior is deliberate and auditable.
- `magic-market-transport` already has cross-crate HTTP policy and gate tests.
- Existing numeric tolerances encode distinct source/business units and must
  not be replaced with one global epsilon.

## Technical Decisions
| Decision | Rationale |
|----------|-----------|
| Introduce one checked packet cursor for TDX | Central bounds/offset handling prevents each parser from reimplementing partial checks. |
| Require declared-count agreement | A complete empty response is distinct from a truncated nonempty response. |
| Reuse shared request-start pacing for Exchange | The shared gate releases reservation locks before waiting and I/O. |
| Keep an Exchange TLS adapter where required | Shared Rustls-only transport cannot silently replace the admitted native-tls diagnostic path. |
| Add a declarative admission registry checked by compliance tooling | Avoid provider dependencies in the Router while detecting flag/document drift. |
| Add parameterized checked tolerance primitives plus named call-site policies | Reuse mechanics without erasing units or source precision. |

## Fixed Time and Numeric Policies

- Core now owns the only Unix-seconds-to-fixed-offset Gregorian conversion in
  the workspace. China timestamps are a named `+08:00` specialization with
  checked arithmetic and RFC3339 year bounds.
- `ClockTime` accepts exactly `HH:MM:SS`; BR-019 compares the typed clock
  instead of re-parsing bytes or relying on lexical assumptions.
- The Eastmoney, THS, CNInfo, CLS, and ThePaper copies were removed. Their
  source-specific failure paths remain typed rather than returning an empty
  batch.
- `NumericTolerance` rejects invalid components and non-finite operands. Money
  cents, order-book sums, Tencent percentage points/trade amounts, Sina
  top-of-book prices, and SZSE source precision retain their previous units
  and acceptance boundaries.

## Release-profile evidence

- The final deterministic benchmark measures 64-row TDX bar/variable parsing,
  JSON decode plus checked normalization, bounded zlib decompression, and a
  zlib compression/decompression roundtrip. Every profile/run produced
  identical per-workload checksums.
- On clean revision `8c8e9b5`, thin LTO with one codegen unit improved the
  geometric combined median by only 1.29%. TDX parsing improved 1.98%, JSON
  normalization improved 2.51%, zlib decompression regressed 3.03%, and the
  roundtrip improved 3.56%.
- No workload crossed the 5% regression budget and the binary shrank 4.85%,
  but combined improvement missed the required 5%. The candidate failed
  closed and `[profile.release]` was removed.
- A second clean hardened-runner session on `d9555c6` measured 7.25% combined
  improvement with no workload regression and a 4.87% smaller binary. The
  first session used `8c8e9b5`; parser hot-path code, the runner, and default
  binary size changed before the second session. The sessions are not directly
  comparable and provide insufficient evidence for a workspace-wide profile.
  Both raw sessions are retained and the repository continues to use Cargo's
  default release profile.

## Issues Encountered
| Issue | Resolution |
|-------|------------|
| Some audit claims combined real code smells with false consequences | Separate factual presence, production reachability, and remediation priority. |
| The generic symmetric relative formula is slightly wider than Tencent's reference-relative trade contract | Evaluate Tencent's existing two-percent-plus-CNY-100 threshold against the expected amount, then use an absolute checked tolerance so the source boundary does not move. |

## Resources
- `docs/business_rules.md`
- `docs/ENGINEERING_RULES.md`
- `docs/PERFORMANCE_RESULTS.md`
- `tools/compliance/check.sh`
- Provider integration documents under `docs/integrations/`
# Implementation Findings

## TDX packet boundary

- Both security and index bar fixtures demonstrate a protocol-authorized
  optional four-byte tail. The strict parser therefore accepts exactly zero or
  four tail bytes and rejects every other tail length.
- A single checked cursor now distinguishes valid encoded zero from truncated
  or unterminated data. Public fixed-width readers and `get_price` return
  `Result`, so workspace callers can no longer silently manufacture zero.
- Security lists, declared bar batches, transaction batches, and realtime quote
  batches reject a missing later record atomically. Historical minute data has
  no count field, so its six-byte header is required and every started
  price/auxiliary/volume tuple must finish.

## Exchange request pacing

- `magic-exchange-rs` now uses the shared reservation gate. The mutex protects
  reservation arithmetic only; waiting and HTTP execution occur after release.
- The source-specific `ureq` adapter remains because Exchange exposes explicit
  Rustls/native-tls selection. Compatible endpoint and request-header
  validation now pass through shared transport value contracts.
- Existing CFFEX integration coverage and the new transport test observe
  spaced actual starts with overlapping slow injected I/O, while TLS backend
  error evidence remains unchanged.

## BR-009 registry

- Source discovery found 17 public `*_ADMITTED` constants across 11 Provider
  crates. The TSV registry now binds every constant to its exact Provider,
  boolean, evidence document, date/counts, and blocker.
- WallstreetCN's older evidence predated the uniform threshold. Two consecutive
  live probes and a three-call serial load probe passed on 2026-07-29, so its
  existing `true` capability remains evidence-backed without an exception.
- The offline checker rejects missing/unknown/duplicate rows, boolean drift,
  bad evidence paths, sub-threshold admitted counts, and absent false-row
  blockers. It is called by the existing compliance gate.

## Final review hardening

- TDX financial archives now reject incomplete index tables, offsets into the
  index, non-four-byte report widths, and any out-of-bounds declared report
  atomically. Cumulative price arithmetic and unsigned transaction domains are
  checked rather than wrapped or cast.
- Exchange request and response validation now uses one source-specific shared
  `EndpointPolicy` per call, including exact query-key and media-type
  allowlists. CFFEX uses the same contract, and a cloned-client test proves
  that slow I/O overlaps after request starts are spaced.
- BR-009 accepts only Git-tracked regular source, registry, and evidence files
  inside the repository. Untracked files and symlinks fail closed.
- Release-profile evidence now requires four exact workloads, exact iteration
  counts, complete metadata, consistent throughput, five named runs, and an
  unchanged clean Git revision. The fourth workload measures zlib compression
  plus decompression.
- Follow-up review found that raw TDX bars/minutes/quotes could still expose
  negative derived prices or signed volumes before Core normalization. These
  public parsers now reject negative price results, require every quantity to
  fit `u32`, and reject rather than wrap opaque varints stored in unsigned
  fields.
- Benchmark schemas now require an integer version, exact run schemas, full
  Rust/Cargo version formats, and fixed default/candidate descriptions. The
  runner rejects inherited Rust/Cargo build variables, rechecks untracked
  files after execution, and permits in-repository artifacts only below the
  ignored `target/` tree.
- The final runner builds both profiles from an isolated archive of the
  captured commit. It uses an isolated Cargo home linked only to offline cache
  directories and rejects automatic Cargo configuration in the source or any
  ancestor, so a transient worktree edit or user config cannot alter one
  profile while evidence still records the original revision.
- Inline critical-module tests were moved to `tests/internal/` through
  `#[path]` modules so coverage evidence measures production lines rather than
  counting test bodies. Additional Core and TDX boundary cases raised the
  measured workspace coverage to 87.88% and critical-path coverage to 95.01%,
  satisfying the unchanged 80% and 95% release thresholds.
