# People's Bank of China integration

## Capability state

The exactly cataloged 2024 money-supply table is admitted. Social financing
and regional-series admission flags remain false and those families are
unsupported.

## Official host and paths

The only cataloged table is the individually audited 2024 HTML resource under
`https://www.pbc.gov.cn/eportal/fileDir/diaochatongjisi/resource/cms/`.

## Request and response ceilings

HTML is capped at 2 MiB, table grids at 100 rows by 16 columns, cell text at
512 characters, and request starts at one-second intervals.

## Identity, unit, missing, and source-time semantics

The exact bilingual Money Supply title/unit, twelve `YYYY.MM` columns, empty
presentation tail and merged-cell M2 → M1 → M0 hierarchy are mandatory. Zero
is present; blank/dash is missing. The current 2024 page exposes January
through October values and explicit blank November/December cells, which are
returned as `Missing`. The page does not prove a release timestamp, so
`source_at` remains absent.

## Authentication or usage-rights boundary

No login or protected document route is used. Uncataloged year URLs are never
guessed.

## Deterministic tests

Fixtures cover the audited 19×16 Excel-HTML grid, bilingual title/unit/series
rows, cell-span provenance, missing/zero, presentation tail, note/history
boundary, accepted charset declarations and malformed bytes.

## Live and load admission evidence

On 2026-07-29, two consecutive release live probes each strictly parsed the
official page and returned the requested 12 M2 monthly observations. A
three-call serial load probe passed the one-second request-start gate with no
overlap. `economic_series=true` therefore applies only to the exact 2024
`money-supply` catalog; social financing and regional series remain false.

## Explicit unsupported operations

Uncataloged years, social-financing PDF/XLSX ingestion, regional series and
generic document scraping are unsupported.
