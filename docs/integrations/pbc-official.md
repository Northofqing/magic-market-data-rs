# People's Bank of China integration

## Capability state

The exactly cataloged 2024 money-supply table and the exact 2025 Q1 regional
social-financing flow workbook are admitted. National social-financing articles,
other periods, stock tables, and uncataloged regional workbooks remain unsupported.

The 2026-08-13 follow-up audit admitted only the official regional workbook.
The national monthly flow report is rendered as an official article and is not
inside the registered transport contract.

## Official host and paths

The only cataloged table is the individually audited 2024 HTML resource under
`https://www.pbc.gov.cn/eportal/fileDir/diaochatongjisi/resource/cms/`.

The admitted regional source is:

- the official 2025 Q1 regional-flow workbook at
  `/diaochatongjisi/fileDir/resource/cms/2025/05/2025051514404575389.xlsx`,
  linked by the PBC article titled `2025年一季度地区社会融资规模增量统计表`.

It has a separate exact-path policy from the HTML table, accepts only
`application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`, forbids
redirects, and caps the response at 256 KiB. The checked-in official response is
26,227 bytes with SHA-256
`0ebb3780b5fac74c2aa10a14a6996138a19c39b1050c498da6632ffd19c5e6eb`.
Before XML parsing, the ZIP directory is limited to 64 unique safe-path entries,
2 MiB per expanded entry, and 4 MiB expanded in total; encrypted, multi-disk,
ZIP64, duplicate and unsupported-compression entries fail closed.

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
`money-supply` catalog. The separate social-financing and regional flags apply
only to the exact workbook contract documented below; no other family is
promoted by the HTML evidence.

## Explicit unsupported operations

Uncataloged years, national social-financing article/PDF ingestion, other
regional workbooks and generic document scraping are unsupported.

## Regional social-financing contract and evidence

The parser requires the exact visible 2025 Q1 sheet plus the exact hidden
template sheet, bilingual title/unit/header cells, the preliminary-statistics
note, all 31 source-named regions, and nine numeric columns. It produces one
quarterly observation per region and requested column with unit `亿元人民币`,
scale `100 million yuan`, preliminary revision, and no inferred region code.
Blank, textual, fractional, missing, malformed or oversized value cells fail
closed; zero is preserved as a present value.

On 2026-08-13, two independent release-mode live probes each returned 31
complete `AFRE_FLOW` records with the expected unit and source values. A
three-request serialized load probe returned `admitted` with one-second minimum
request spacing and no overlap. Fixture tests additionally cover all 279
region-column observations, source spelling, negative/zero values, request
bounds, truncation, malformed ZIP and oversize rejection.

`SOCIAL_FINANCING_ADMITTED` and `REGIONAL_SERIES_ADMITTED` therefore mean only
the exact `regional-social-financing-flow` namespace and exact 2025 Q1 workbook;
they do not admit national social-financing, monthly flow, stock, growth-rate,
or arbitrary regional families.
