# Official publication service integration evidence

## Scope and authority

The 2026-10-01 user instruction “接入吧” authorized the service integration and
runnable collector described in the
[Gate A design](../superpowers/specs/2026-10-01-official-publication-service-integration-design.md).
This worktree remains uncommitted. No production service was restarted or
deployed and no continuous collector or OS task was installed. Existing runtime
instances require their own approved update and capability/build-identity check.

## Implemented contract

- Append-only v1 `OfficialPublications=64` and `OfficialPublication=65`; all
  preceding operation values and GlobalNews v2 remain intact.
- Full production registry exposes both operations for Nbs, Pbc, Ndrc, Mof,
  Miit, Mofcom, Nea and Csrc, with precise column scopes. Default publisher is
  explicitly Nbs. Gacc stays blocked even with `allow_unadmitted=true`.
- Each response retains one complete source-native listing/original envelope;
  `source_at` is absent, publication label/origin/precision remain native,
  response hashes and URLs remain attached, and batch identity binds the
  complete envelope. `complete` is bounded-response completeness only.
- Listing and original handlers share one source client and its request gate.
  HTTP timeout can be shortened to at least one second and is capped at the
  admitted 15 seconds. Composition's shared transport error-enum dependency is
  registered; there is no new provider-local HTTP/TLS stack.
- `official-news-collector` serially polls the same operations, appends and
  flushes independent success/failure NDJSON events, and sleeps after each
  completed round. Default limit is five and delay is 300 seconds. Explicit
  `--rounds` bounds verification. The sidecar lease excludes other collectors
  without preventing journal readers; incomplete tails and file errors fail.
- Public client bundle defaults to `2026-10-01.1` with 65 RPCs and native v1
  metadata. It does not include runtime credentials or a deployed build identity.

## Live verification

[Metadata-only live record](2026-10-01-official-publication-service-live.json)
contains source code hashes, observed times, exact URLs, response hashes,
native label precision/origin and independent batch identities. Full original
text is retained in the temporary collection journal, not copied into this
evidence record.

| Path | Bound | Observed outcome |
|---|---|---|
| Real HTTP/2 gRPC fixture with production official handlers | 8 sources × 2 rounds × (one listing + one original), limit 1 | 32 successful queries; source_at empty, provider identity and native content retained |
| Standalone collector executable | 8 sources × 2 rounds × (one listing + one original), limit 1, 60-second delay | 32 successful queries; 32 complete flushed NDJSON events; zero provider failures |
| Timer | completed round 1 to first round 2 observation | 60.598868 seconds, satisfying the configured minimum |
| Gacc over gRPC | preferred Gacc and allow_unadmitted=true | UNIMPLEMENTED; no source HTTP request |

Total: **64 successful official HTTP responses**, 16 listings and 16 originals
on each path. The fixture gRPC listener is private ephemeral loopback and uses
the production query handlers. Existing production Bearer/mTLS enforcement was
not changed; this live test does not claim deployment of the new methods.

Commands used:

```text
cargo test -p magic-market-grpc-server --bin magic-market-grpc-server official_publications_live_grpc_probe --locked --offline -- --ignored --nocapture
cargo build -p magic-market-composition --bin official-news-collector --locked --offline
official-news-collector --output <temporary-journal.ndjson> --interval-secs 60 --limit 1 --rounds 2
```

The live test is explicitly ignored in default workspace checks; it was run
separately and passed. The focused interface test suite passed 125 tests before
the final sidecar-reader test was added. Final workspace evidence below includes
that additional test.

## Explicit intermediate failures and correction

The first compile identified a shadowed collector request-construction helper
and omitted `all_unadmitted` blocker arguments. Both were corrected using the
compiler diagnostics. The next test run identified the shared transport's
existing one-second minimum and two expected new Gacc blocked registrations;
timeout validation and exact capability inventory tests were corrected.

The first compliance run rejected the unregistered Composition shared-transport
dependency. The approved design and `http-transports.tsv` now register it as
shared transport with no direct HTTP/TLS stack. A links-check shell invocation
once exited with native Windows process status 3221225477 and no diagnostics;
the subsequent standalone invocation passed. No link or checker was weakened.

Clippy then required the one-byte newline array comparison to use a byte string.
`[b'\n']` became `*b"\n"`; the comparison and journal policy are identical.
The live record's source/binary hashes preserve the exact probed version, with
the equivalent post-probe source adjustment recorded separately. Final checks
use the adjusted source.

## Gate C final checks

| Final command/check | Outcome |
|---|---|
| `cargo test --workspace --all-targets --locked --offline` | 1,978 passed, zero failed, three ignored; 250 test groups; 122.12 seconds |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | passed, zero warnings; 4.63 seconds |
| `cargo fmt --all --check` | passed |
| `cargo doc --workspace --no-deps --locked --offline` | passed; generated 42 documentation entry files; 136.12 seconds |
| `tools/compliance/check.sh` | passed: 60 provider capability rows, 29 HTTP dependency boundaries, TDX compatibility/native and gRPC registries |
| `tools/docs/check_links.sh` | passed after adding this evidence and its native-contract link |
| `tools/docs/build_client_bundle.ps1` | passed: version 2026-10-01.1, exactly 65 RPCs, both native schema versions 1 |
| Public client bundle hash verification | all seven manifest files verified; bundled Proto/API bytes match the worktree; no deployed build identity or credentials |
| `cargo deny check` | advisories, bans, licenses and sources passed; five existing duplicate-version warnings (getrandom, hashbrown, syn, webpki-roots, windows-sys) |
| `git diff --check` | passed |

The three default ignored tests include the new bounded live gRPC probe, which
was separately executed and passed. The other two remain existing workspace
ignores. Final tests include the sidecar lease/readability and incomplete-tail
tests, source failure isolation, no-I/O request rejection, blocked Gacc, native
evidence retention, shared list/original pacing and complete RPC/enum parity.

The source admission rows remain the existing eight independently admitted
publisher contracts; this increment does not broaden any native website scope,
admit Gacc, invent a publication instant or claim complete history.
