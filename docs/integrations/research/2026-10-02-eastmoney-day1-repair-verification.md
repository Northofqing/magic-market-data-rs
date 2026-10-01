# Eastmoney Day1 source-only repair verification (2026-10-02)

The coordinating Mac task accepted the exact [Gate A scope and public seam](../../superpowers/specs/2026-10-02-eastmoney-daily-flow-repair-design.md) under the existing human-authorized repair task. Only Day1 request routing and fixed data-only JSONP framing changed. Minute1, BoardFlows, Core/protobuf, dependencies, endpoint allowlists, TLS, HTTP limits, pacing and deployment did not change. The repository-requested autonomous-long-task skill is unavailable; no claim is made that it ran.

## Deterministic verification

The exact first-party 380-byte fixture SHA-256 is `f5382c64c38ad33650c94f2cc4b6c0b27ebf8a4458ef9fe2f623ac3250e4a395`. A fixture-local Git attribute disables newline normalization to preserve those original bytes across platforms.

The public `FundFlowSeries::fund_flow_series` regression was run before the implementation change. It failed with `Decode("expected value at line 1 column 1")`, exit 1, one failed test. After the minimal Day1 change it passed, with exact SZ 300005 identity, September 29/30 dates, five CNY tier values, percent ratios and record/batch source evidence. Additional public rejection/whitespace tests passed; these additional validation cases are not claimed as separate red-green regressions.

Commands and actual outcomes:

- `cargo test -p magic-eastmoney-rs --lib fund_flow::tests --locked --offline`: 11 passed, 0 failed.
- `cargo test -p magic-eastmoney-rs -p magic-market-composition -p magic-market-grpc-contracts -p magic-market-grpc-server --lib --locked --offline`: 219 provider + 67 composition + 6 contract tests passed. The server has a binary test target, not library tests.
- `cargo test -p magic-market-grpc-server --bin magic-market-grpc-server --locked --offline`: 26 passed, 0 failed, 1 live test ignored. Repository test RPCs are deterministic test fixtures, not the candidate's deployed same-build acceptance.
- `cargo fmt --all -- --check` and all-target Clippy for those four packages with `--locked --offline -- -D warnings`: passed.
- `tools/compliance/check.sh`: initially rejected the new untracked Rust probe. After explicitly staging only task files, all five registries passed (60 admission capabilities and 29 HTTP crates); no registry counts were advanced.
- `tools/docs/check_links.sh`: this round's normal Bash invocation exited 0. Earlier unexplained native Bash `0xc0000005` exits remain in the previous delivery; this successful run does not establish their root cause or erase them.
- `git diff --cached --check`: initially detected a literal whitespace-only fixture line in a test. Representing the same bytes with escaped string segments removed that diagnostic; formatting and tests were rerun.

## Actual bounded Rust Provider failures

`cargo run -p magic-eastmoney-rs --example fund_flow_day1_probe --locked --offline` used the normal constructor, exact SZ 300005, Day1 limits 1 and 2, no injected transport, no credentials and no retry. At Unix milliseconds `1790888925466` and `1790888925716`, both reads failed in the HTTP status-line stage with `peer closed connection without sending TLS close_notify`. Exit 1; no normalized batch or raw HTTP body was obtained. The compiled provider file SHA-256 was `0a7dac92f56e0030a2b4e280831592a584a70f27bc44e3eb86cff753aea165f3`.

The shared request gate recorded 2 starts, maximum concurrency 1 and minimum actual start gap 1.0002855 seconds. These are failure pacing observations, not successful serial-load admission. The existing historical 2 live + 3 serial admission counts are unchanged and do not certify this new path.

## Release and handoff boundary

This is a source-only candidate. Production 50051 was not replaced. The previously rejected isolated 50056 startup remains `rejected: blocked by policy`, with no precise matched rule returned. No alternate launcher, TLS relaxation or production replacement was attempted. Normally authorized same-build Health/Capabilities and business RPC acceptance, plus repeated real new-path successes, remain required before release. Compiling or hashing a candidate binary is not deployment or a live Health identity.

Mac's third public evidence packet was independently read and verified: all 6 files, both original QueryResponse byte lengths/hashes, each CanonicalPayload/data slice, all 8 parts per query, issued/observed request equality, response request_id, the published domain/length-prefixed correlation algorithm, and the compiled CSV calendar algorithm/vector/limit. SH 688561 contains 11 requested dates; SH 688277 contains only July 30 and lacks the other 10 requested dates. Both server envelopes say complete=true. Keep ObservedOnly / NotAdmitted / CoverageUnknown / PITNotCertified. The CSV is exact compiled repository bytes, not captured SSE publication/revision evidence; raw status/trailer capture explicitly says Absent.

Raw test transcripts, actual Provider failure transcript and the independent public-byte audit script/result are retained separately in the authorized Mac shared handoff. They contain no tokens, private keys, certificates or authentication metadata.
