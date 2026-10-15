# Admitted providers outside the `_ADMITTED` constant registry

`admissions.tsv` indexes public Rust constants ending in `_ADMITTED`. The
crates below register production-admitted routes in the gRPC composition but
predate the constant convention: they have no `_ADMITTED` constant and no
registry row. **They are not unadmitted** — absence from `admissions.tsv` must
not be read as a block, and none of them appears in
[`unadmitted-provider-routes.md`](unadmitted-provider-routes.md). Their
admission rests on the contract tests and bounded probes recorded at
integration time. The newest bounded probe batch (2026-10-07,
`docs/evidence/2026-10-07-upstream-live-probes-230223/`) is recorded per crate
below; it is a single run, not a continuous health monitor.

| crate | production wiring (`grpc_production.rs`, snapshot 2026-10-14) | 2026-10-07 bounded probe |
| --- | --- | --- |
| `magic-sina-rs` | 623, 977, 1190 | failed |
| `magic-tencent-rs` | 523, 3312-3408, 3341 | failed |
| `magic-ths-rs` | 2127, 2168, 2193, 2220, 2245 | completed |
| `magic-cninfo-rs` | 735, 4923 | completed |
| `magic-cls-rs` | 821 | completed |
| `magic-thepaper-rs` | 840 | completed |
| `magic-gov-rs` | 787 | completed |

Adding `_ADMITTED` constants and admission gates for these crates is deferred
until downstream consumers of their operations are confirmed, because the
gates would shrink currently advertised capability until fresh probes pass
(`2026-10-14-admission-index-coverage-gate-a-design.md`, decision D2c: (b)
first, (a) only after consumer confirmation). This index grants no data rights
and executes no probes.
