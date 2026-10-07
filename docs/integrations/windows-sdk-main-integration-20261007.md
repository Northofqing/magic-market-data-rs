# Windows SDK main integration — 2026-10-07

## Pinned source and intent

The direct user requested complete source integration and full rollout. These
are separate outcomes: source integration does not activate a runtime or waive
Gates C/D. The local candidate was
`c72c907d1f12583fe5072642519c2549f5f0c317`; remote main was
`92afdbe83009457558193a4f03954f8491119291`. Their merge-base is
`0554654dcc03718f272580d3d7bed447de5892fb`, with one/21 unique commits.
Main is not an ancestor of the original candidate. Non-fast-forward overwrite
and force push are not the integration strategy.

## Conflict decisions

Preserve both commit histories. Eight conflicting paths were resolved by intent:

- README retains the upstream official-source scopes, collector guidance and
  dated deployment documentation, plus the new source-quality disclosure note.
- The production gRPC implementation retains the candidate's coverage handlers
  and external test-module link. Upstream's gRPC source equals the original
  candidate snapshot, so its inline tests are already retained in the extracted
  test file; they must not be duplicated back into measured production sources.
- Final deployment Markdown/JSON remain the exact upstream final observations.
  The initial deployment remains a separate record, with its link corrected to
  the initial JSON. Different process IDs, requests and binary identities are
  not synthesized into one observation, nor represented as a current deployment.
- CNInfo/API documentation keeps the later bounded coverage contracts; API
  documentation also preserves upstream's historical deployed-source identity.
- Client-bundle tooling keeps upstream's default `2026-10-01.3` and the candidate's
  additional coverage documents/metadata. A future qualified deployment must
  select an appropriate explicit bundle version and matching identities.

No source contract, admission registry, HTTP/TLS policy, coverage glob/threshold,
checker or workflow is relaxed by this merge resolution. Unrelated untracked
research notes remain outside the staged source integration.

## Review-driven correction

The pinned whole-range review used separate Standards and Spec axes. Standards
found no hard violations in inspected hunks and two optional maintainability
judgments; its historical JSON/test/tool sampling limits are explicit. Spec
found two existing disclosure defects: rejection of valid incomplete prefixes
and report/forecast/repurchased-share shortcuts bypassing title ambiguity.

The [corrective design](../superpowers/specs/2026-10-07-disclosure-discovery-regression-repair-design.md)
keeps the original disclosure seam, limits and payloads. Source-batch
`source_complete` is exposed rather than relabeled true; malformed records and
acquisition failures remain atomic errors. All supported title kinds participate
in one ambiguity decision. This does not add historical technology search,
PDF financial extraction or daily authoritative trading status.
The complete disclosure test module is relocated to `tests/internal/` with its
original cases intact, preventing test-body lines from counting as production.

## Qualification boundary

The last completed original coverage CI was for 7174: overall 84.96% meets 80%,
critical 88.89% fails 95%. It is neither a new successor measurement nor release
approval. A successor requires fresh original checks and exact-HEAD CI evidence.
No production process, certificate, credential, port or controller is changed by
these source edits. Native Bash mechanism evidence also remains distinct from
an unperformed runtime repair or vendor attribution.
