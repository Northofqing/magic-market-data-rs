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

**Update 2026-10-15:** the news/disclosure families of five of these crates are
now registered with `_ADMITTED` constants and admitted `admissions.tsv` rows:
`magic-cls-rs` `GLOBAL_NEWS_ADMITTED`; `magic-cninfo-rs` `ANNOUNCEMENTS_ADMITTED`
/ `MARKET_ANNOUNCEMENTS_ADMITTED` / `INVESTOR_QUESTIONS_ADMITTED`;
`magic-gov-rs` `POLICY_DOCUMENTS_ADMITTED`; `magic-sina-rs`
`INSTRUMENT_NEWS_ADMITTED`; `magic-thepaper-rs` `GLOBAL_NEWS_ADMITTED`. Their
remaining operations stay outside the constant convention until downstream
consumers are confirmed (news-coverage decision D2c/D4a).

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

## Literal-admitted routes without constants or ledger rows (update 2026-10-15)

Three routes are registered in the production composition with an `admitted(...)`
literal but have no `_ADMITTED` constant and therefore no `admissions.tsv` row.
They are **not unadmitted** and do not belong to
[`unadmitted-provider-routes.md`](unadmitted-provider-routes.md); they are
recorded here because the ledger (2026-10-15 decision D1a/D2b of
`2026-10-14-admission-index-coverage-gate-a-design.md`) keys rows by
`(crate, constant)` only, and minting constants would require fresh admission
evidence (>=2 bounded live probes and >=3 serial loads) that does not exist yet.

| route | crate | production wiring (`grpc_production.rs`, snapshot 2026-10-15) | evidence status |
| --- | --- | --- | --- |
| `GlobalIndices` | `magic-sina-rs` | 642 - `admitted(..., "Sina", "verified global index identities")` | bounded probe evidence recorded in `docs/integrations/sina-web.md` (6 verified global indices); no BR-009 probe counts |
| `ForeignExchange` | `magic-sina-rs` | 660 - `admitted(..., "Sina", "verified currency-pair identities")` | same document (8 verified currency pairs); no BR-009 probe counts |
| `NorthboundDaily` | `magic-exchange-rs` | 2186 - `admitted(..., "Hkex", "official Shanghai- or Shenzhen-connect northbound daily statistics for an exact date")` | `docs/integrations/exchange-official.md` 2026-07-22 live evidence; no BR-009 probe counts |

Source: overseas market Gate A design section 6
(`2026-10-14-overseas-market-data-gate-a-design.md`, repository-local), which
registers these routes as input to the admission-index decision. Minting their
constants and rows stays deferred to a separate decision; this table is the
explicit listing both documents asked for and grants no data rights.
