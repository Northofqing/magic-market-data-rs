# Eastmoney Day1 response-bound successor

Mac independently accepted the previous source-only packet, then identified that a positive lmt query parameter alone did not prove a bounded returned batch. Its fourth public feedback packet contains three files; their lengths/hashes and manifest SHA-256 `db81314929092ba4958e6cf03592f1f2d14220dca797be2cdfe8b71f047b983f` were independently verified on Windows. This does not authorize deployment, network/TLS changes or a workaround for the prior denied 50056 startup.

## Confirmed seam, diagnosis and red-green

The already-confirmed public `FundFlowSeries::fund_flow_series` seam replays the original native 380-byte two-row fixture for SZ 300005, now with limit 1. On source 1bd1ae7 it returned two records with complete=true; the regression genuinely failed. This proves a local boundary defect, not a live observation of the service returning two rows for limit 1 (the real limit-1 read failed before status).

Request construction preserves the Core limit and emits it as lmt. The private parser does not receive it, and BatchContext finishes all parsed rows as a strict batch. Thus the missing response cardinality guard, not gRPC projection or a lost Core field, caused the over-limit public result. The working popularity provider already rejects responses exceeding its caller limit as Protocol errors.

The minimal successor keeps private JSON/JSONP parsing and all existing source validation intact, then checks Day1's actual normalized row count before returning it. Any count above the caller limit fails the entire public call with a typed Protocol error containing only counts. It never truncates rows or returns a complete subset. The limit-1/two-row test is now green; the native limit-2/two-row golden fixture and the existing limit-1/one-row case remain green. Minute1 behavior, BoardFlows, Core/protobuf, HTTP/TLS/dependencies, pacing and admission counts are unchanged. No speculative contract or architectural changes are included.

## Public source handoff

Mac lacks the previous delta Git bundle's prerequisite 67c832e. The next delivery therefore identifies this successor explicitly and exports a self-contained exact Git source ZIP containing the unmodified workspace Cargo.toml, Cargo.lock and all 41 workspace crates, with complete local path dependencies. ZIP entries receive individual original byte lengths and SHA-256 hashes; the extraction must be independently rechecked with locked Cargo metadata and normal public Provider build/tests. This is workspace-source closure, not a vendored registry cache or bundled Rust toolchain. Registry crates remain pinned by the original lockfile and require an existing cache or a normal authorized Cargo fetch.

The export is a source-only allowlist, excluding Git history, ignored/untracked files, target/runtime, credential/config files, auth keys, certificates and environment files. Configuration source code such as src/config.rs is ordinary public Rust code, not a private runtime config. The canonical source commit, archive/lockfile hashes, input manifest, local path closure audit and extraction/build results must accompany it. Old cached sources, binary hashes and source-only metadata must not be relabeled as deployed Health or live admission.

The previous candidate release file d047570a... belongs to 1bd1ae7, not this successor. Actual same-build candidate Health/business RPC remains pending a normal policy-permitted path. D14/D17/D20 missing-day reasons, authority coverage, source revision/PIT and R08 Confirmed lifecycle evidence remain open; this cardinality correction and source export do not close them. Announcement source truncation remains fail-closed under the existing versioned coverage contract.
