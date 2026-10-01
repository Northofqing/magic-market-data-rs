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
| Started at UTC | `2026-10-01T13:42:08.1679192+00:00` |
| Service version | `0.2.0` |
| Source snapshot revision | `67c832e43f36f188e4d769f409691c0b1d9a2ea2` |
| Contract descriptor SHA-256 | `abf28a3e0028488a7579da4d961e1a7c1408482bdc0500122c1956d225e480cf` |
| Running binary SHA-256 | `9302a3036c7ed272fdf24dd84b4a66881ab682649e9af76b68ef62b2ad083854` |
| Previous binary SHA-256 | `517e0b4c31bb42330f4bc2a0395e3af385a164212414a65775bed40c9eb87ae3` |
| Build target | `x86_64-pc-windows-msvc` |
| Backup directory | `target/runtime/archive/official-news-20261001T131255Z/` |

The source revision is an immutable **local runtime snapshot**, created with a
separate temporary Git index and retained under `refs/codex/runtime-builds/20261001T131255Z`. It includes
the current integrated worktree. The working branch remains at
`0554654dcc03718f272580d3d7bed447de5892fb`; its index was not changed. This is a workstation runtime
update, not a published release or a push. The build used a new isolated Cargo
target directory, `--release --locked --offline` and the explicit Windows target.
All 583 Rust/Cargo/configuration input files were checked
against the frozen source, normalizing only Git line endings. The five fixes already deployed in `4e4995f8d3f2c7cd504d1dec0f238e6d4b4fc02c`
were retained in this combined snapshot, including FuturesDelivery v2 and the
Eastmoney bank/forex article metadata hosts. No Rust source was changed after
this final combined snapshot.

The previous server executable and all nine hashed public client-bundle files
plus their manifest are backed
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
probe passed without a service-code change; it took 28.498 seconds.

Machine-readable endpoint results are in
[the deployment record](2026-10-01-official-publication-service-deployment.json).
The first isolated snapshot temporarily passed the eight-source news checks but
was subsequently rejected as a final deployment: the existing runtime came from
five commits beyond the working branch and exposed FuturesDelivery v2. A real
v2 request against the first snapshot failed as v1-only. The original server and
its complete hash-verified public bundle were restored while those five committed
fixes were merged with the news integration. The final combined runtime also
passed all 12 monthly FuturesDelivery v2 responses (48 planned records) and six
negative cases, including explicit rejection of v1 and unsupported years.
The intermediate deployment is preserved as
[historical evidence](2026-10-01-official-publication-service-initial-deployment.md).

## Final verification and client delivery

| Check | Final result |
|---|---|
| `cargo fmt --all --check` | passed |
| `cargo test --workspace --all-targets --locked --offline` | 1,979 passed; zero failed; three ignored; 250 groups; 467.34 seconds |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | passed; 78.3 seconds |
| `tools/compliance/check.sh` | passed: 60 capability rows, 29 HTTP boundaries, TDX/native/gRPC registries; 177.24 seconds |
| `cargo doc --workspace --no-deps --locked --offline` | passed; 42 entries; 486.76 seconds |
| Final isolated optimized server build | passed; 1,409.05 seconds |
| Client-bundle builder | version 2026-10-01.3; 65 RPCs; retained FuturesDelivery v2; official schemas v1 |
| Public client bundle | all nine manifest hashes passed; exact deployed source, descriptor and binary identities; Proto/API/futures bytes match current sources |
| Protected runtime configuration/credentials/helpers | all 20 hashes unchanged |

A Git Bash document-check invocation once exited with native Windows status
3221225477 and no diagnostics. A native checker using the same rg pattern and
file-existence rules passed all 261 local links without weakening the checked-in
wrapper. The final wrapper result after these records is appended separately.
The final server copy initially encountered a transient Windows sharing violation
just after process termination; the previous binary was restored. With no runtime
processes remaining, exclusive executable access was confirmed and the install
was retried with a bounded sharing-violation wait. The successful install and full
endpoint probe above followed that retry.

The dependency graph and Cargo lockfile did not change while retaining the
previously deployed compatibility fixes; the earlier successful dependency-policy
evidence remains applicable. The new full workspace checks above replace the
initial integration-only counts for this final combined source.

Final checked-in document wrapper `tools/docs/check_links.sh` passed after all
records were written (2.08 seconds). `git diff --check` passed. A final authenticated
health call still returned live/ready with the final combined build identity;
port 50051 was owned by server PID 10964, with Agent and monitor running.
The standalone collector remained absent.
