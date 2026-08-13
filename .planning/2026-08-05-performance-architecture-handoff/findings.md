# Findings & Decisions

All measurements were taken on 2026-08-05 at revision `06b4d0f`, host
`Darwin 25.5.0 x86_64`, `rustc 1.95.0`. This is a shared development machine
outside the repository's provenance harness. Treat every number as an
engineering signal, not as Gate D evidence.

## Assessment Summary

Contract design and engineering governance are strong. Implementation coverage
and I/O orchestration are not.

- 31 crates, 449 Rust files, 129,528 lines.
- `magic-market-core` is 20,700 lines across 19 modules with 59 traits and 274
  public items, and depends on no HTTP library. The layering
  core → router → composition → provider is real and enforced.
- After excluding `#[cfg(test)]` regions, production code contains only 33
  `unwrap`/`expect`/`panic!` sites total, 26 of them `Mutex::lock().unwrap()`
  in `magic-tdx-rs`. Zero `TODO`/`FIXME`/`HACK` comments workspace-wide.
- `magic-market-transport/src/http.rs:64-96` rejects duplicate headers,
  credential-bearing headers, authority/framing/hop-by-hop headers, and control
  characters, on top of a closed `MediaType` allowlist and `redirects(0)`.
- `tools/compliance/` ships checkers with their own unit tests, and
  `docs/integrations/*.tsv` are machine-verifiable registries.

The gap: `magic-market-composition` is 288 lines of source and binds exactly one
route (Eastmoney top-N rankings), while core declares 59 traits and router
declares 30+ `FailoverChain` aliases. Contract design runs far ahead of
composition.

## Corrections to Earlier Claims in This Session

Recorded so the next reader does not inherit them.

1. **"Two TLS stacks are linked in" — false.** `ureq 2.12.1` and
   `reqwest 0.13.4` share the same `rustls 0.23.42` and `ring`; 52 crates are
   common to both trees. ureq-only additions are just `ureq`, `webpki-roots`,
   and `zmij`. `native-tls` exists only behind the non-default
   `magic-exchange-rs` feature. Consolidating HTTP is a compile-time win of
   three crates, not a binary-size problem.
2. **"gzip gives 5–10x" — overstated.** Measured 3.5–3.8x on numeric-heavy
   market JSON and 2.4x on the small repository fixtures. Use 3.8x.
3. **"The 5 MB TDX financial archive would benefit from compression" — false.**
   `crates/magic-tdx-rs/src/net/finance_client.rs:501` requests
   `Accept: application/zip` and the body is unpacked by
   `extract_financial_zip` at line 65. That payload is already compressed.
   Compression only helps plaintext JSON and HTML endpoints.

## P0 — Ten crates hold a mutex across network I/O

`magic-market-transport/src/gate.rs:5-8` documents the correct contract:

> The mutex protects only reservation arithmetic. It is released before the
> caller sleeps, and this type never performs network I/O.

The hand-rolled ureq providers do the opposite. In
`crates/magic-baidu-rs/src/lib.rs:166-189` the guard is taken at line 168, the
pacing sleep happens at line 174 while holding it, the full HTTP round trip
runs at line 182 still holding it, and `drop(last_started)` only occurs at
line 188.

Affected sites:

```
baidu:168   cls:180   cninfo:306   gov:167       iwencai:279
jin10:197   thepaper:189   ths:224   wallstreetcn:111   yonhap:303
```

All ten use a `Duration::from_secs(1)` interval (`baidu:22`, `cninfo:84`,
`ths:90`, and the equivalent constant in the others).

The twelve crates that already sit on `magic-market-transport` — cfets, exchange,
fred, imf, nbs, pbc, sec, stcn, worldbank, xinhua, yicai — use `RequestGate`
correctly. tencent, eastmoney, and sina have no pacing primitive at all.

### Measurement

Six threads, 24 requests, 100 ms interval, 120 ms simulated RTT, with the real
`RequestGate` as the control arm:

| Arm | Elapsed | Throughput |
| --- | ---: | ---: |
| Lock held across I/O (current) | 2.99 s | 8.03 req/s |
| `RequestGate` (target) | 2.49 s | 9.63 req/s |

Throughput gain is only **1.20x**. The severe result appears when one request
stalls to the 10-second timeout ceiling:

| Arm | Total | p50 | p95 | max |
| --- | ---: | ---: | ---: | ---: |
| Lock held across I/O | 12.87 s | 124.87 ms | **11.12 s** | 12.50 s |
| `RequestGate` | 10.48 s | 504.63 ms | **604.48 ms** | 10.11 s |

**p95 improves 18.4x.** In the current code one stalled endpoint blocks every
other concurrent caller of that client for the full `DEFAULT_TIMEOUT` of 10 s
(`baidu/src/lib.rs:18`), including calls bound for unrelated endpoints. The p50
column is not comparable across arms because the queueing discipline differs.

### Precedent

This exact bug class was already fixed once in this repository. The
`.planning/2026-07-30-p0-architecture-hardening` slice released
"Release the synchronous TDX outer pool-handle lock before socket I/O", and the
remediation is visible at `crates/magic-tdx-rs/src/net/client.rs:598`:

```rust
let pool = Arc::clone(&*sync::lock(&self.pool, "connection pool handle")?);
```

Clone out of the guard, let the guard drop, then perform I/O. The ten ureq
crates are the same defect in a different layer, and `RequestGate` is the
already-tested remediation.

### Data-acquisition risk — medium, and mitigable to zero

Requests, responses, and parsing are unchanged, so returned data is identical.
The side effect is rate. The buggy path spaces requests by `interval + RTT`;
`RequestGate` spaces them by exactly `interval`. At a 1 s interval and 120 ms
RTT that is **0.89 → 1.00 req/s, about 12% faster against undocumented public
endpoints**. `docs/PERFORMANCE_RESULTS.md` already warns these figures are
"not a vendor SLA or a safe sustained request rate".

**Mitigation: raise the interval to `1.2s` during migration** so the observed
upstream rate is unchanged, and take the head-of-line fix for free. Reduce it
afterwards only with live-probe evidence.

### Governance

No new Gate A is required. `docs/integrations/http-transports.tsv` already
carries these crates as `legacy-direct`, `migration_status=legacy`, with the
reason `existing provider-local ureq stack; migrate behind shared transport`.
This is execution of a registered migration. The registry rows must move to
`shared` as each crate lands, and `tools/compliance/check_http_transports.py`
enforces that.

## P1 — Compression is disabled workspace-wide

`http.rs:469-472` sets `no_gzip()`, `no_brotli()`, `no_zstd()`, `no_deflate()`,
and `validate_request` at `http.rs:297-307` rejects any `Accept-Encoding` other
than `identity`.

### Measurement

Synthetic full-market ranking payloads shaped like the Eastmoney and THS
endpoints, gzip level 6:

| Payload | Raw | gzip | Ratio | Saved |
| --- | ---: | ---: | ---: | ---: |
| 100-row snapshot | 22,153 B | 6,382 B | 3.5x | 71.2% |
| 1,000-row page | 222,183 B | 59,780 B | 3.7x | 73.1% |
| 5,400-row full A-share | 1,204,593 B | 318,958 B | 3.8x | 73.5% |

The 35 repository fixtures total 33,729 B → 13,988 B (2.4x), lower only because
they are truncated samples where gzip framing overhead dominates.

### Data-acquisition risk — medium

gzip is lossless, so decompressed bytes are identical and provenance and BR-033
freshness are untouched. Three real risks:

1. **Body limits must be reworked.** `MAX_CONFIGURED_BODY_BYTES` is 16 MiB of
   *wire* bytes today. Under compression a 300 KB body can expand to gigabytes.
   The decompression stream must be wrapped in `take(MAX_DECOMPRESSED)` with a
   separate compressed-byte cap. This is mandatory, not optional, and it is the
   reason the current policy exists.
2. **Upstream behaviour can vary with `Accept-Encoding`.** Some CDN and WAF
   configurations return different or corrupted bodies. This requires per-provider
   live-probe evidence under Gate D; it cannot be enabled workspace-wide at once.
3. **Blast radius is small.** Outside `magic-market-transport` (6 references,
   policy plus tests) only two sites mention the policy:
   `magic-sec-rs/src/transport.rs:132` and
   `magic-tdx-rs/src/net/finance_client.rs:501`. **Do not touch the TDX site** —
   it is a hand-written HTTP/1.1 request over raw TCP and its payload is already
   a zip.

## P2 — The benchmark harness cannot resolve its own gate

Reproduced the documented baseline with
`cargo run -p magic-tdx-rs --example parse_bench --release`, matching
`docs/PERFORMANCE_RESULTS.md` closely (tdx_bar_parse 1.650 s vs 1.645 s
recorded). Then built a candidate with `CARGO_PROFILE_RELEASE_LTO=thin` and
`CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1` into an isolated `CARGO_TARGET_DIR`,
leaving the workspace unmodified. Four alternating runs per profile:

| Workload | Default median | LTO median | Delta | Default spread | LTO spread |
| --- | ---: | ---: | ---: | ---: | ---: |
| tdx_bar_parse | 1354.1 ms | 1392.9 ms | +2.9% | 27.5% | 39.9% |
| json_normalize | 1038.4 ms | 951.2 ms | −8.4% | 21.6% | 37.1% |
| zlib_decompress | 746.5 ms | 588.6 ms | −21.1% | 32.3% | 39.2% |
| zlib_roundtrip | 4362.3 ms | 3862.4 ms | **−11.5%** | 7.9% | 8.8% |

All four checksums matched across profiles
(`4287391093950792928`, `7267965373649679376`, `440610000`, `197516000`), so
both binaries performed identical work.

**The finding is not the LTO number.** Run-to-run spread of 22–40% on three of
four workloads exceeds the 5% effect being tested, so those rows are
statistically unusable. Only `zlib_roundtrip`, with 8% spread, yields a
trustworthy −11.5%.

The harness invests heavily in provenance — read-only `git archive` snapshot,
isolated Cargo home, tree-digest recheck, porcelain verification before and
after — but nothing in variance control. There is no inner-loop repetition with
min-of-N, no CPU pinning, no outlier rejection, and only five outer runs. A
workload with 30% spread will fail the 5% gate forever regardless of how many
provenance checks are added. The repository's fail-closed decision was correct;
the instrument is what needs work.

**Recommendation:** fix sampling first (inner-loop min-of-N, discard warmup
iterations, report MAD alongside median), then re-run the gate. The evidence
suggests thin LTO would clear 5% once it is measurable.

## P3 — `InstrumentId` allocation

`crates/magic-market-core/src/instrument.rs:21-24` stores `code: String`, and
line 33 reads:

```rust
let code = code.into().trim().to_owned();
```

That is two heap allocations plus a free per construction. There are 85
`.clone()` sites across provider sources, and router request types are
`[InstrumentId]`.

### Measurement

2,000,000 iterations against the real type, compared with an inline
`[u8; 8] + len` `Copy` candidate:

| Metric | Current | Inline `Copy` | Gain |
| --- | ---: | ---: | ---: |
| `size_of` | 32 B | 11 B | −66% |
| Construct | 579.6 ms (290 ns/op) | 31.1 ms (15.6 ns/op) | **18.6x** |
| Clone | 178.9 ms (89 ns/op) | 3.1 ms (1.6 ns/op) | **56.8x** |

Checksums matched across all four arms.

### Data-acquisition risk — HIGH, and the obvious implementation is wrong

**`InstrumentId::new` enforces no length limit.** It checks only for an empty
string and control characters (`instrument.rs:26-47`). The repository already
carries 8-character codes — `10000001`, `10012127`, `01010503` are SSE option
contract codes, exactly 8 digits.

An `[u8; 8]` buffer therefore has **zero headroom**. Any longer identifier —
Hong Kong codes, futures contracts, a future asset class — would move from
"works today" to "constructor rejects it", which is silent data loss surfacing
only at runtime.

**Revised recommendation:**
- For this major, take only the allocation half: change line 33 to a single
  allocation via `AsRef<str>`. No API change, no type change, no rejected
  instruments, roughly 40% of the gain for near-zero risk.
- Defer the `Copy` conversion to the next major, and size it `[u8; 16]`
  (`size_of` 20 B, still `Copy`, still allocation-free) or a small-string type
  with heap spill. It is a breaking change regardless, because the lifetime
  semantics of `code() -> &str` change.

## P4 — Untyped JSON navigation

292 `serde_json::Value` references and 652 `.get()`/`.as_str()`/`.as_array()`/
`.as_f64()` navigation calls in provider sources, against 251 derived
`Deserialize` implementations. A meaningful share of providers materialise a
full DOM and then pick fields out of it.

### Measurement

698,486-byte payload, 5,400 rows, 400 iterations, identical output checksums:

| Path | Per parse | Gain |
| --- | ---: | ---: |
| Untyped `Value` navigation | 27,987.5 µs | — |
| Typed with `#[serde(borrow)]` | 5,984.4 µs | **4.68x** |

### Data-acquisition risk — HIGH if implemented with `&str`

Verified experimentally:

```
unescaped     &str = Ok("贵州茅台")          Cow = Ok("贵州茅台")
\uXXXX form   &str = Err(invalid type:       Cow = Ok("贵州茅台")
                     string "贵州茅台",
                     expected a borrowed string)
```

`#[serde(borrow)] &'a str` **fails outright** on `\uXXXX`-escaped strings,
because an escaped string must be decoded into a fresh buffer and cannot be
borrowed. Chinese endpoints — sina and ths in particular — commonly return
escaped JSON. A naive migration would cause **total parse failure** for those
providers.

**`Cow<'a, str>` is mandatory.** It borrows when unescaped and owns when
escaped, and retains essentially all of the 4.68x.

Secondary risk: typed structs fail on missing or null fields unless declared
`Option` or `default`, whereas `Value` navigation may currently tolerate them.
Upstream schema drift would shift from silent degradation to hard failure.
Under Gate B's strict-completeness requirement hard failure is arguably the
correct behaviour, but it changes availability and must be assessed per
provider rather than applied uniformly.

## P5 — Regex recompiled per call

`crates/magic-tdx-rs/src/profile/parser_f10.rs:89` compiles
`Regex::new(r"【(\d+\.[^】]+)】").unwrap()` inside `split_sections()`, so the
pattern is rebuilt on every call. Regex compilation costs two to three orders
of magnitude more than matching. Lines 79 and 99 additionally clone both title
and content. The workspace contains only two `OnceLock`/`LazyLock` uses total.

Zero data risk; it caches a constant pattern.

## End-to-End Projection

Composing the measured parts for "fetch a 5,400-row full A-share ranking and
convert to domain records". Component costs: transfer 1,204,593 B → 318,958 B,
parse 28.0 ms → 6.0 ms, identifier construction 1.57 ms → 0.08 ms, and −11.5%
applied to the CPU portion for LTO.

| Link | Current | Optimized | Gain | Bottleneck after |
| --- | ---: | ---: | ---: | --- |
| 100 Mbps | 126 ms | 31 ms | 4.08x | network |
| 20 Mbps | 511 ms | 133 ms | 3.85x | network |
| 5 Mbps | 1,957 ms | 516 ms | 3.79x | network |
| 1 Mbps | 9,666 ms | 2,557 ms | 3.78x | network |

**End-to-end lands at 3.8–4.1x, and P1 contributes roughly 90% of it.** The
library is thoroughly I/O bound: at 20 Mbps the entire CPU side is 29.6 ms of
511 ms, about 6%. The changes with the largest multiples are the least valuable
in wall-clock terms.

## Other Confirmed Observations

- Router failover is strictly sequential (`router/src/router.rs:246`), so
  worst-case latency is the sum of every source timeout. No hedged requests,
  no parallel cross-validation.
- No circuit breaker exists anywhere; `grep CircuitBreaker` returns nothing. A
  failing source is re-attempted at full timeout on every route.
- No `tracing` or `log` dependency in any manifest. `RouteOutcome.attempts`
  records dispositions but nothing is emitted at runtime.
- 128 `async fn` exist and all are in `magic-tdx-rs`; the other 25 providers are
  blocking, as `docs/integrations/async-blocking.md` acknowledges.
- 35 provider error enums with heavily overlapping variants: `InvalidRequest`
  28x, `Unsupported` 24x, `Transport` 24x, `Protocol` 21x.
- `HttpsTransport::new` is character-for-character identical between
  `magic-baidu-rs/src/lib.rs:65-80` and `magic-jin10-rs/src/lib.rs:71-86`, and
  the `AgentBuilder` block repeats across 14 crates. Consolidation removes an
  estimated 1,500–2,000 lines.
- `USER_AGENT` is defined in 9 places with inconsistent values, ranging from
  `"magic-imf-rs/0.2"` to a spoofed `"Mozilla/5.0 (Macintosh; ...)"`.
- `router/src/adapters.rs` is 1,547 lines opening with a 30-line import block of
  60+ types; it is a candidate for splitting by domain.
- 25 of 31 crates lack a `description`, which crates.io requires, yet none set
  `publish = false`. All 31 are pinned at `0.2.0`. `README.md` is 63,524 bytes.
- 39 `#[allow(...)]` suppressions warrant individual review.

## Reproduction

Baseline, from the workspace root:

```bash
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets
cargo run -p magic-tdx-rs --example parse_bench --release
```

Candidate release profile, without modifying the workspace:

```bash
CARGO_TARGET_DIR=/tmp/mmd-lto \
CARGO_PROFILE_RELEASE_LTO=thin \
CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
  cargo run -p magic-tdx-rs --example parse_bench --release
```

The pacing, identifier, JSON, and escape experiments ran from throwaway crates
under `/tmp/gbench`, `/tmp/idbench`, and `/tmp/jbench`, each with a path
dependency on the corresponding workspace crate. They were scratch artifacts and
are not preserved. Rebuilding them is straightforward from the descriptions
above; if they are wanted permanently they belong behind a bench feature rather
than in the default build, and they must not become downstream path
dependencies.
