# Integration evidence index

Provider integration documents describe source-specific endpoint, schema,
provenance, admission, and explicit-failure contracts.

`admissions.tsv` is the machine-readable BR-009 index for every public Rust
constant ending in `_ADMITTED`. An admitted row requires an existing evidence
document, a canonical live-evidence date, at least two bounded live probes, and
at least three serial load calls. A false row requires an explicit blocker.
Source, registry, and evidence files must be Git-tracked regular files inside
the repository; symlinked or untracked evidence is rejected. `-` is the
canonical placeholder for an intentionally empty optional field.

`status` and `last_live_date` record the admission EVENT, not current health.
`blocker_class` classifies each blocked row against a fixed vocabulary
(`entitlement`, `anti_bot`, `source_discontinued`, `source_attribution`,
`source_fields`, `semantics_unproven`, `contract_drift`, `transport_tls`,
`template_unverified`; multiple classes join with `;`). `last_probe_utc` /
`last_probe_status` carry the newest bounded live-probe batch outcome per row
(`completed`, `failed`, `mixed`, `blocked`, `diagnostic`); the 2026-10-07 batch
is recorded under
[`../evidence/2026-10-07-upstream-live-probes-230223/summary.json`](../evidence/2026-10-07-upstream-live-probes-230223/summary.json).
Admitted providers that predate the `_ADMITTED` constant convention are indexed
in [`provider-coverage-index.md`](provider-coverage-index.md).
Five of them registered their news/disclosure families on 2026-10-15; the index
records which families remain outside the constant convention.

Run the drift check without network access:

```bash
python3 tools/compliance/check_admissions.py
bash tools/compliance/check.sh
```

The registry does not execute probes, grant data rights, or turn a diagnostic
implementation into a production capability. It binds already-recorded
evidence to the exact Rust constant so source and documentation changes cannot
silently diverge.

## Runtime and architecture boundaries

- [`async-blocking.md`](async-blocking.md) explains how to call the current
  synchronous HTTP providers from Tokio without blocking executor workers.
- [`official-domestic-publications.md`](official-domestic-publications.md)
  records the eight admitted original-publication scopes and blocked customs source,
  native date precision, unified service/gRPC contracts and the opt-in serial collection journal.
- [`http-transports.tsv`](http-transports.tsv) records the reviewed production
  HTTP dependency boundary and is checked against tracked Cargo manifests.
- [`tdx-local-terminal.md`](tdx-local-terminal.md) records automatic TDX process
  discovery, the fixed official TQ-Local HTTP transport and the zero-Python/DLL
  decision.
- [`tdx-local-terminal-compatibility.tsv`](tdx-local-terminal-compatibility.tsv)
  is the version/schema evidence inventory; executable identity is provenance
  and the bounded loopback health probe decides runtime compatibility.
- News data is bounded by design ([BR-066](../business_rules.md)): a news query
  inspects at most 20 records per admitted Provider, no result claims complete
  history or full-market coverage, and per-source field differences are declared
  in each registration's `exact_scope` string.
- [`grpc-external-api.md`](grpc-external-api.md) is the authenticated external
  transport contract; [`grpc-derived-products.md`](grpc-derived-products.md)
  fixes the five append-only composed-product payload schemas.

The HTTP transport registry is an architecture-control inventory. It does not
grant provider admission; `admissions.tsv` remains the BR-009 capability
evidence index.

The local-terminal compatibility and native-boundary checks are also offline:

```bash
python3 tools/compliance/check_tdx_local_compatibility.py
python3 tools/compliance/check_tdx_native_boundary.py
```
