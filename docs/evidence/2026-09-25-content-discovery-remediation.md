# Bounded content discovery remediation evidence (2026-09-25)

## Implemented scope

- `GlobalNews` has an explicit `WallstreetCn` default rather than relying on
  registration order; pinned Provider requests are unchanged.
- iWencai production `SemanticSearch` rejects `News`, `Announcement` and
  `General` with typed `Unsupported` before transport. The Report-only
  admission and strict HTTPS URL contract remain intact.
- The composition crate exposes bounded candidate discovery over admitted
  current news windows and Cninfo announcement windows, with a runnable
  `content_discovery` example. Candidate labels never replace source evidence.

## Deterministic verification

- The iWencai pre-I/O regression test was red before the fix: three unadmitted
  channel requests reached transport (`left: 3`, `right: 0`). It passed after
  the channel gate was added.
- The default-provider regression test was red before the registry selection
  method existed, then passed with the configured `WallstreetCn` default and a
  pinned Eastmoney assertion.
- `cargo test -p magic-iwencai-rs -p magic-market-service -p magic-market-composition --all-targets --offline`
  passed on 2026-09-25. The composition candidate fixtures distinguish Rubin
  from unrelated NVIDIA news, Muse Spark from Muse agent, annual from half-year
  reports, and shareholder reduction from repurchased-share reduction. A bad
  source record rejects that source's candidate set rather than returning a
  seemingly complete partial subset.

## Bounded read-only live observations

The first sandboxed news run had eight source failures caused by its restricted
network context, so it was repeated outside the sandbox. This is an
environment comparison, not a source reliability statistic. Discovery now
reports `NoSourceSucceeded` for that case and its example exits nonzero.

`content_discovery news "NVIDIA|英伟达" "Rubin|鲁宾"` inspected latest windows from
WallstreetCn (20), Cailianpress (20), ThePaper (12), Yicai (20), Jin10 (17),
Yonhap (14) and Eastmoney (20). XinhuaFinance failed in that one attempt. No
Rubin candidate appeared in these windows. The operation does not query a
historical archive, so this does not prove that the sources never covered Rubin.
After the status change, the same bounded query again returned no Rubin hit;
seven sources were inspected, XinhuaFinance failed, and the result reported
`PartialSourceFailure` rather than an unqualified empty success.

`content_discovery issuer SH 600519 2026-01-01 2026-09-25 annual` inspected 62
Cninfo announcements and returned the source-linked 2025 annual report,
English version and summary, all published 2026-04-17.

`content_discovery issuer SZ 002131 2025-01-01 2025-12-31 repurchased-share-reduction`
inspected 142 Cninfo announcements and returned the source-linked October
plan and December progress announcements for reduction of repurchased shares.
Those titles are not reinterpreted as ordinary shareholder reductions or as
evidence of a completed sale amount.

## Unresolved admission and coverage

- iWencai News has no confirmed entitlement, Skill ID/version, quota or
  display/storage permission. It remains production-unadmitted.
- No new licensed historical technology source, unified gRPC operation,
  persistent cross-source index, event detail extractor or completeness claim
  is added in this slice. Historical Rubin and Muse retrieval remains open.
- Some upstream iWencai News results contain `http://` article links. They
  remain strict-batch failures until a separately designed, source-proven HTTPS
  or explicit partial-result contract exists.
