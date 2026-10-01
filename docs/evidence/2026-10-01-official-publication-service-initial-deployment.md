> Intermediate deployment: rolled back after detecting the existing FuturesDelivery v2 compatibility gap. The first eight-source endpoint checks below remain historical evidence only. Final deployment evidence is recorded separately.

# Official publication service deployment — 2026-10-01

The configured Windows gRPC runtime was updated and started after the user's
explicit request to update the service. Before the update, the server and Agent
were not running and the old PID files were stale. The existing endpoint,
authentication token, certificates, startup scripts and helper binaries were retained.

## Deployed identity and rollback material

| Field | Verified value |
|---|---|
| Endpoint | `https://10.211.55.3:50051` |
| TLS server name | `magic-market.local` |
| Started at UTC | `2026-10-01T12:47:01.5510667+00:00` |
| Service version | `0.2.0` |
| Source snapshot revision | `bd96679bc29c19da4f55612dc7b201e4bfe42241` |
| Contract descriptor SHA-256 | `abf28a3e0028488a7579da4d961e1a7c1408482bdc0500122c1956d225e480cf` |
| Running binary SHA-256 | `b286f27ad69a814be3aa665d130f4732c0fb95f1714ee37753aeb005af4e7e4b` |
| Previous binary SHA-256 | `517e0b4c31bb42330f4bc2a0395e3af385a164212414a65775bed40c9eb87ae3` |
| Build target | `x86_64-pc-windows-msvc` |
| Backup directory | `target/runtime/archive/official-news-20261001T122551Z/` |

The source revision is an immutable **local runtime snapshot**, created with a
separate temporary Git index and retained under `refs/codex/runtime-builds/20261001T122551Z`. It includes
the current integrated worktree. The working branch remains at
`0554654dcc03718f272580d3d7bed447de5892fb`; its index was not changed. This is a workstation runtime
update, not a published release or a push. The build used a new isolated Cargo
target directory, `--release --locked --offline` and the explicit Windows target.
All 583 Rust/Cargo/configuration input files were checked
against the frozen source, normalizing only Git line endings. No Rust source was
changed during this deployment task.

The previous server executable and seven public client-bundle files are backed
up. Private connection material remains in the existing runtime directory.
To roll back, stop this runtime, restore its backed-up executable, then use the
existing startup script; the matching public bundle files can also be restored.

## Actual runtime verification

- Authenticated `GetHealth` returned `live=true`, `ready=true` and no identity error.
  Its source revision matches the source snapshot; descriptor and executable hashes
  match the files produced by this isolated build.
- `GetCapabilities` returned 65 registered operations, 63 with an admitted and
  available handler. The two new operations have 18 exact source rows: eight
  admitted publishers and blocked Gacc for each operation.
- Remote reflection returned exactly 65 `MarketDataService` methods, including
  `OfficialPublications` and `OfficialPublication`.
- Each of the eight admitted sources returned one listing and its selected original
  through the deployed mTLS + Bearer endpoint: **16 successful official-source
  requests**. Provider identity, native schemas/version 1, original URL, actual
  publication date/label, response hashes, nonempty original text and empty
  `QueryResponse.source_at` were checked.
- Gacc with `allow_unadmitted=true` still returned `UNIMPLEMENTED`.
- Existing `GlobalNews` from WallstreetCn returned the unchanged v2 news record with
  its own evidence.
- Missing Bearer was rejected. Missing client certificate was rejected before an
  application response; an equivalent Python TLS client with the configured
  certificate reached HTTP/2. Server CA and hostname verification remained enabled.
- The existing Agent reconnected; listener state at this probe was
  `agent_connected_production`. This does not claim new account access or terminal
  market data beyond the listener's explicit state.
- All 20 protected configuration, credential and helper-binary
  files retained their exact hashes. Their contents and private hashes are not
  included in public evidence.

| Source | Validated listing rows | Selected original date | Text characters | Result |
|---|---:|---|---:|---|
| Nbs | 15 | 2026-09-30 | 4998 | passed |
| Pbc | 15 | 2026-09-29 | 1826 | passed |
| Ndrc | 25 | 2026-09-30 | 678 | passed |
| Mof | 10 | 2026-08-26 | 4024 | passed |
| Miit | 24 | 2026-09-30 | 445 | passed |
| Mofcom | 15 | 2026-09-29 | 562 | passed |
| Nea | 10 | 2026-09-30 | 569 | passed |
| Csrc | 18 | 2026-09-28 | 650 | passed |

The standalone timer collector was neither replaced nor started. These tests
verify on-demand server RPCs. Source scopes remain those recorded in
[the native contract](../integrations/official-domestic-publications.md).

## Verification provenance and diagnostic corrections

The integrated code's complete pre-deployment checks are preserved in
[the integration evidence](2026-10-01-official-publication-service-integration.md):
formatting, workspace tests, Clippy, documentation generation, compliance,
document links, dependency policy and compatibility evidence. The code inputs
were subsequently frozen and compared as described above. This deployment added
an isolated optimized build and actual endpoint tests; earlier loopback probe
evidence is not represented as deployed endpoint evidence.

A private probe initially used Python's invalid starred-expression syntax and was
corrected before installation. The first endpoint probe expected grpcurl to expose
a TLS alert; grpcurl instead reported its dial deadline for missing client identity.
A separate TLS probe observed `TLSV13_ALERT_CERTIFICATE_REQUIRED`, while some
Windows reads reported connection abort (10053). A same-client certificate control
confirmed successful HTTP/2. Python 3.13's stricter default X.509 policy also
rejected this existing local certificate for a missing Authority Key Identifier;
the helper then used normal `PROTOCOL_TLS_CLIENT` chain and hostname verification,
matching the existing client trust policy. No server certificate or TLS policy was changed.

The helper also initially sent `application/json`; the server correctly rejected
it because the transport requires `application/json; charset=utf-8`. Its batch-ID
assertion initially omitted the documented `official-` prefix. Both helper checks
were corrected using the current contract and actual response. Failed endpoint
probe records are retained privately beside the build log. The final complete
probe passed without a service-code change; it took 17.444 seconds.

Machine-readable endpoint results are in
[the deployment record](2026-10-01-official-publication-service-deployment.json).
Public client-bundle delivery and final documentation checks are recorded below
when verified.
