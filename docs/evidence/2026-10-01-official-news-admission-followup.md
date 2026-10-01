# Official publication admission follow-up — 2026-10-01

## Authorized scope and source evidence

The user requested admission of the five remaining domestic publishers. The
[Gate A follow-up design](../superpowers/specs/2026-10-01-official-news-admission-followup-design.md)
records the source-specific contract changes. The
[official-source research](2026-10-01-official-news-admission-followup-research.md)
records exact MOF, MIIT and NEA first-party requests, layouts and date conflicts.
The first four publishers' original evidence remains in
[the first-batch report](2026-10-01-official-news-first-batch.md).

MOF now selects the comprehensive department's policy-release column, MIIT
selects leadership activities, and NEA selects the news-center's two five-row
work-update windows. These sources do not prove admission of the previously
blocked broad financial-news, latest-policy or dynamic policy columns. No HTTP
original is upgraded or silently dropped. The shared HTTP registry mode and
all timeout/body/TLS/redirect settings remain unchanged.

All four new original profiles use their visible publication date/time labels
with explicit `publication_label_origin`. MOF, NEA and CSRC retain date precision;
MIIT retains minute precision. Metadata, event dates and generation timestamps
do not overwrite those source labels. List references retain their own labels
and evidence; originals and lists are separate observations.

## CSRC exact aggregate scope

The first native diagnostic failed while validating the complete source page,
before applying caller limits, with `InvalidRequest(article URL is outside the
exact source profile)`. A fresh normal-TLS GET of the
[website's exact page-one API](https://www.csrc.gov.cn/searchList/a1a078ee0bc54721ab6b148884c784a8?_isAgg=true&_isJson=true&_pageSize=18&_template=index&_rangeTimeGte=&_channelName=&page=1)
at `2026-10-01T10:16:41.648506Z` returned SHA-256
`a701ca397061fbcca66ef2fa12bb90afc5c7d6fc83ddce2761ad104fd88e2fc1`.
It declares `page=1`, `rows=18`, `channelId=a1a078ee0bc54721ab6b148884c784a8`,
`relateSubChannels="true"`, `total=3197`. All 18 results were inspected: 15
originals use `c100028`, and three use same-host `c106311` leadership paths.
Protocol-relative hrefs resolve under the existing HTTPS source URL; no explicit
HTTP href is changed.

The verified
[leadership original](https://www.csrc.gov.cn/csrc/c106311/c7657131/content.shtml)
visibly labels its publication date `2026-09-07` and uses the same original
layout. Admission explicitly includes only those two numeric original paths;
the parser also verifies aggregate/page identity and `min(total,18)` result
cardinality. Each final live round samples the first two `c100028` originals
and one `c106311` original. This remains one current aggregate page, not a
3,197-record history or an arbitrary-channel crawler.

## GACC remains blocked

Three distinct default-validation observations support retaining the blocker:

- The previous independent Python HTTPS probe failed certificate-chain
  verification; its source templates were never admitted.
- Windows `Invoke-WebRequest`, with ordinary default TLS validation,
  `TimeoutSec=15` and `MaximumRedirection=0`, failed with
  `RemoteCertificateNameMismatch, RemoteCertificateChainErrors` on
  `https://www.customs.gov.cn/`.
- A temporary Rust probe linked to the workspace's existing
  `magic-market-transport` and requested the same HTTPS root with a 15-second
  timeout, 2 MiB HTML bound, no redirects, no proxy and ordinary certificate
  validation. It returned `Network(request failed before a valid HTTP
  response)` and no source response bytes. The typed transport error does not
  expose a more detailed TLS cause; it must not be described as a separately
  proved Rust certificate-chain failure.

No trust-store mutation, disabled certificate validation, plaintext fallback
or provider-local TLS dependency was introduced. The
[official customs online portal](https://online.customs.gov.cn/)
shows service notices, which do not establish the requested customs macro-news
list/original contract. A service-maintenance feed cannot substitute for that
scope. Formal GACC listing/original calls continue to reject before I/O.

## Verification progress

Expected contract-test failures were observed before their fixes: the old
CSRC original returned `Unsupported`; old MOF/MIIT/NEA profiles rejected the
new fixtures; CSRC aggregate leadership paths were rejected and page-identity
mutations were initially accepted. The implementation resolves the observed
source contracts and preserves each earlier failure as evidence.

## Native formal live admission

The final `live_probe` build completed two formal rounds and three same-client
serial list calls for every admitted source. All **74 HTTP responses** passed
request-bound transport, source-row validation, original body/title/date
checks and evidence-origin checks. CSRC additionally sampled one leadership
original per round. No failing run or stderr was hidden. The persisted
[metadata-only live record](2026-10-01-official-news-admission-followup-live.json)
retains all list/reference and original evidence, response hashes, observation
times, date labels/origins and body lengths; it does not duplicate full articles.

| Source | Complete source rows | Round 1 UTC / listing SHA-256 | Round 2 UTC / listing SHA-256 | Serial list calls |
|---|---:|---|---|---:|
| nbs | 15 | `2026-10-01T10:27:39.2450497Z` / `40064f27414db06a7d2eaf62c5fac94d177ed2707db9827b9a679642fe772fcd` | `2026-10-01T10:27:42.8034001Z` / `40064f27414db06a7d2eaf62c5fac94d177ed2707db9827b9a679642fe772fcd` | 3 |
| pbc | 15 | `2026-10-01T10:27:38.9809022Z` / `7da4b61e81c171531f107fa20ca56cac9b66ab192956f616fabeb0e24c805e18` | `2026-10-01T10:27:42.384969Z` / `7da4b61e81c171531f107fa20ca56cac9b66ab192956f616fabeb0e24c805e18` | 3 |
| ndrc | 25 | `2026-10-01T10:27:39.0011159Z` / `6482ab4e14cd2e59449ecfe6ee566764fc0a7cad5aa7b7af7ba97bd73829e1e4` | `2026-10-01T10:27:42.2838131Z` / `6482ab4e14cd2e59449ecfe6ee566764fc0a7cad5aa7b7af7ba97bd73829e1e4` | 3 |
| mof | 10 | `2026-10-01T10:27:39.0782013Z` / `4755667a48492b136df7af6ceabc69beced14c95ee5df55e122e919e60921319` | `2026-10-01T10:27:42.5781064Z` / `4755667a48492b136df7af6ceabc69beced14c95ee5df55e122e919e60921319` | 3 |
| miit | 24 | `2026-10-01T10:27:47.7900017Z` / `60a5d6cd0b9efc46e37c9b68d2ec5eeb760e724adbf4a695be7fab1156833031` | `2026-10-01T10:27:51.1065207Z` / `60a5d6cd0b9efc46e37c9b68d2ec5eeb760e724adbf4a695be7fab1156833031` | 3 |
| mofcom | 15 | `2026-10-01T10:27:48.079266Z` / `868949db408800b9adde5ce9fcb4971eb65d41ba78f9685ec84855c306185968` | `2026-10-01T10:27:51.0824214Z` / `739d5d0d4fde3385b54aab86eae92254de0ac86c0b084fa6319be2b551bf23c7` | 3 |
| nea | 10 | `2026-10-01T10:27:48.1475853Z` / `8a02ac3a217b68641bef1cda11d13ad8bf9a6386862fa15b405609b957c654d6` | `2026-10-01T10:27:51.3410107Z` / `8a02ac3a217b68641bef1cda11d13ad8bf9a6386862fa15b405609b957c654d6` | 3 |
| csrc | 18 | `2026-10-01T10:27:48.8437838Z` / `a701ca397061fbcca66ef2fa12bb90afc5c7d6fc83ddce2761ad104fd88e2fc1` | `2026-10-01T10:27:53.0372474Z` / `a701ca397061fbcca66ef2fa12bb90afc5c7d6fc83ddce2761ad104fd88e2fc1` | 3 |

## Final Gate C

Checks ran against the final admitted implementation on 2026-10-01:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | Passed |
| `cargo test --workspace --all-targets --locked --offline` | 1,967 passed, zero failed, two ignored, 249 test groups; 189.25 seconds |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Passed without warnings; 35.70 seconds |
| `cargo doc --workspace --no-deps --locked --offline` | Passed; 41 crate indexes generated; 16.53 seconds |
| `bash tools/compliance/check.sh` | Passed: 60 capability entries, 28 HTTP crates, TDX compatibility/native boundary and gRPC registry |
| `bash tools/docs/check_links.sh` | Passed |
| `cargo deny check --hide-inclusion-graph` | Advisories, bans, licenses and sources passed; five existing duplicate-version warnings remain |

The native crate has 15 passing contract tests. New assertions cover visible
date authority and precision, Chinese MOF date labels, MIIT metadata conflicts,
NEA's two exact groups, all rows beyond caller limits, CSRC aggregate/page
identity, truncated source pages and the two explicit original channels.
GACC's remaining formal admission guard is verified before transport.

Verification logs are retained locally as
`C:\Users\13687\AppData\Local\Temp\codex-news-followup-*.log`.
Git whitespace and final documentation links were rechecked after recording
these results. No source code changed after the verified final build.
The final direct PowerShell invocation of the link script reported exit 1
without diagnostics. A retry of the same script through the existing structured
check runner recorded the actual Bash child exit 0 in 0.66 seconds, in
`codex-news-followup-links-final.log`. No script or link was changed to obtain
that pass; the discrepancy in the direct invocation status was not attributed
to an unproved cause.

## Released native scope

Eight source constants are admitted; GACC remains false with its current
blocker in the registry. Formal listing/original calls are available through
the Rust library only. No unified gRPC route, archival store, full historical
coverage, scheduled collection or derived event pipeline was added in this
admission task. Existing uncommitted workspace work is preserved.
