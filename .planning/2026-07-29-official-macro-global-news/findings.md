# Findings: Official Macro, Global Data, SEC, and Financial News

## Existing Contract Coverage

- `magic-market-core::calendar` models future or announced economic events,
  not historical macroeconomic observations.
- `magic-market-core::global` models a small closed set of current global-index
  and FX snapshots. It does not model economic time series, benchmark-rate
  histories, or official FX fixings.
- `magic-market-core::content::NewsItem` already supports the required
  metadata-only news boundary with provider, source time, observation time,
  and batch evidence.
- No Core filing contract currently represents SEC CIK, accession number,
  form, filing date, report period, and canonical document URL together.
- The current workspace has no NBS, PBC, CFETS, FRED, IMF, World Bank, SEC,
  Xinhua Finance, Yicai, or Securities Times Provider identity.

## Source Scope

- China official data: National Bureau of Statistics, People's Bank of China,
  and CFETS/China Money.
- Global public data: FRED, IMF, and World Bank.
- Company filing metadata: SEC EDGAR public data.
- Financial-news discovery: public first-party metadata from Xinhua Finance,
  Yicai, and Securities Times only when a bounded live audit proves a stable,
  authorized endpoint.
- Tushare paid data, Wind, Choice, iFinD, Bloomberg, licensed research bodies,
  broker-account data, and logged-in browser extraction are excluded from this
  first batch.

## Evidence Boundaries

- A local fetch time is observation evidence, never a fabricated source
  release time.
- Missing numeric observations remain explicit source states and are never
  converted to zero.
- Provider-native indicator codes remain provider-scoped. Similar names from
  two institutions are not silently treated as the same series.
- Pagination is atomic. Partial pages, duplicate identities, contradictory
  metadata, unit changes, and response revisions during one fetch fail the
  batch.
- Public-news implementations expose only title, provider-native or canonical
  identity, canonical URL, publisher, language, topics when supplied, and
  source publication time. Summary and content remain absent.

## Workspace and Baseline

- The isolated feature worktree was created from current `origin/main`.
- `cargo build --workspace --locked --offline` passed.
- `cargo test --workspace --all-targets --locked --offline` passed.
