# Progress

## Status
Assessment complete. No implementation started. Awaiting review before any
Gate A design is opened.

## Session Log

### 2026-08-03 — Architecture assessment
- Established the workspace baseline: 31 crates, 449 Rust files, 129,528 lines.
- Mapped the layering and confirmed core → router → composition → provider is
  enforced, with `magic-market-core` carrying no HTTP dependency.
- Found the transport split: 12 crates on `magic-market-transport`, 14 on
  direct `ureq`, `magic-tdx-rs` on its own TCP stack.
- Counted the duplicated surface: 35 error enums, 14 identical `AgentBuilder`
  blocks, 9 inconsistent `USER_AGENT` constants.
- Confirmed production panic density is very low (33 sites) and there are zero
  `TODO`/`FIXME`/`HACK` comments.
- First `cargo test --workspace --all-targets` aborted with
  `No space left on device`. Diagnosed as a host disk condition, not a defect.

### 2026-08-04 — Verification and performance assessment
- Ran `cargo clean`, freeing the volume, then re-ran the full suite:
  **209 suites, 1574 passed, 0 failed, 2 ignored**. Clippy across all targets:
  **0 warnings**.
- Reproduced the documented `parse_bench` baseline within 0.3% of
  `docs/PERFORMANCE_RESULTS.md`.
- Measured a candidate thin-LTO profile in an isolated target directory.
  Discovered that run-to-run spread of 22–40% makes three of four workloads
  unusable for a 5% gate. Reclassified the harness itself as the blocker.
- Read the transport policy and found compression disabled workspace-wide.
- Found the `RequestGate` contract and the ten crates that violate it.

### 2026-08-05 — Quantification and risk analysis
- Benchmarked the pacing defect against the real `RequestGate`:
  1.20x throughput, and **p95 11.12 s → 604 ms** under a single stalled request.
- Measured compression at realistic payload sizes: **3.8x, 73.5% saved**.
- Benchmarked `InstrumentId`: **18.6x construct, 56.8x clone**, 32 B → 11 B.
- Benchmarked typed versus untyped JSON: **4.68x** on a 698 KB, 5,400-row body.
- Composed the end-to-end projection: **3.8–4.1x, with ~90% attributable to
  compression alone.**
- Performed the data-acquisition risk pass that produced the two blocking
  findings below.
- Corrected three of this session's own earlier claims. See the dedicated
  section in `findings.md`.

## Blocking Findings

Two candidate changes will break data acquisition if implemented the obvious
way. Both were verified experimentally, not inferred.

1. **`InstrumentId` must not use an 8-byte inline buffer.**
   `InstrumentId::new` enforces no length limit today, and the repository
   already carries 8-character SSE option codes (`10000001`, `10012127`,
   `01010503`). An `[u8; 8]` buffer has zero headroom and would begin rejecting
   valid instruments. Use `[u8; 16]` or a small-string type with heap spill, and
   defer it to the next major since it breaks `code() -> &str` lifetimes.

2. **Typed JSON must use `Cow<'a, str>`, never `&'a str`.**
   `#[serde(borrow)] &'a str` fails outright on `\uXXXX`-escaped strings.
   Chinese endpoints commonly return escaped JSON, so a naive migration causes
   total parse failure for those providers. `Cow` handles both forms and keeps
   essentially all of the 4.68x.

## Risk Register

| ID | Risk | Severity | Mitigation |
| --- | --- | --- | --- |
| P0 | Migration raises upstream request rate 0.89 → 1.00 req/s (~12%) against undocumented public endpoints | Medium | Raise interval `1s` → `1.2s` at migration so observed rate is unchanged; lower later only with live-probe evidence |
| P1 | Compression makes the 16 MiB wire-byte cap meaningless and enables decompression bombs | Medium | Wrap the decode stream in `take(MAX_DECOMPRESSED)` plus a separate compressed-byte cap; mandatory, not optional |
| P1 | CDN/WAF may return different or corrupt bodies when `Accept-Encoding` varies | Medium | Enable per provider with live-probe evidence under Gate D; never workspace-wide at once |
| P1 | Touching the TDX financial path would break a hand-written raw-TCP request whose payload is already a zip | Low | Leave `finance_client.rs:501` alone |
| P3 | 8-byte inline code buffer silently rejects valid instruments | **High** | Allocation-only fix this major; `[u8; 16]` or small-string next major |
| P4 | `&str` borrow breaks on escaped JSON | **High** | Mandate `Cow<'a, str>` |
| P4 | Typed structs turn upstream schema drift into hard failure | Medium | Assess per provider; Gate B strict completeness may make this desirable |
| P2 | Benchmark spread of 22–40% cannot resolve a 5% gate | Medium | Fix sampling before drawing any profile conclusion |

## Recommended Sequence

**Step 1 — upstream-neutral, ready to start**
- P0: migrate the ten crates to `RequestGate`, raising each interval to `1.2s`.
  Move their `http-transports.tsv` rows from `legacy-direct` to `shared` as each
  lands. Follow the precedent at `magic-tdx-rs/src/net/client.rs:598`.
- P5: hoist the F10 regex into a `LazyLock`.
- P2: fix benchmark sampling, then re-evaluate the release profile.

**Step 2 — requires per-provider live-probe evidence**
- P1: implement bounded decompression with dual limits, then enable on one or
  two providers before any wider rollout. Do not touch the TDX zip path.

**Step 3 — requires design review**
- P4: migrate hot providers to typed deserialization using `Cow<'a, str>`,
  one provider at a time, each with its existing fixtures as regression.
- P3: land only the single-allocation constructor change now; schedule the
  `Copy` conversion for the next major.

## Open Questions for the Reviewer

1. Should P1 be scoped as its own Gate A design, given it changes the meaning of
   `MAX_CONFIGURED_BODY_BYTES` and relaxes a deliberate security posture?
2. Is the `1.2s` rate-neutral interval acceptable as a temporary value, or
   should the migration carry live-probe evidence for the final interval up
   front?
3. Does P3's `Copy` conversion belong on a `0.3.0` roadmap, or should the
   allocation-only fix be considered sufficient indefinitely?
4. Should the throwaway benchmarks be rebuilt as permanent bench targets behind
   a feature flag, given `AGENTS.md` forbids downstream path dependencies?

## Verification State

| Check | Result |
| --- | --- |
| `cargo test --workspace --all-targets` | 209 suites, 1574 passed, 0 failed, 2 ignored |
| `cargo clippy --workspace --all-targets` | 0 warnings |
| `cargo run -p magic-tdx-rs --example parse_bench --release` | Matches recorded baseline within 0.3% |
| Workspace source modified | **None** |
| Registries modified | **None** |
| `docs/PERFORMANCE_RESULTS.md` updated | **No** — these numbers are not Gate D evidence |

## Housekeeping

- Approximately 3 GB of free disk is required for `target/`. The initial suite
  run failed on a full volume; `cargo clean` resolved it.
- The session todo table carried five unrelated in-progress entries from another
  context (`a10-score`, `dedup-storm`, `preopen-news`, `auction-agent`,
  `sell-t0-loop`). They were left untouched.
