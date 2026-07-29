# SEC EDGAR integration

## Capability state

Filing metadata admission is false. Filing documents and XBRL facts are not
implemented.

## Official host and paths

Requests are limited to `data.sec.gov/submissions/CIK##########.json` and its
checked older-submissions files. `www.sec.gov/Archives/` URLs are returned only
as validated metadata and are never fetched.

## Request and response ceilings

Bodies are capped at 8 MiB, request starts are 500 ms apart, timeout is 15
seconds, and composition is limited to 20 older files and 20,000 validated
rows under one global `max_records` budget.

## Identity, unit, missing, and source-time semantics

CIK, optional ticker, accession, form, filing/report dates, acceptance time and
primary document are exact source facts. Recent/older conflicts fail before
filters; catalog ranges must be ordered and non-overlapping.

## Authentication or usage-rights boundary

`SEC_USER_AGENT` must be descriptive and is always redacted. Status 403 is an
identification failure; 429 remains explicit. There is no retry, login or
attachment/body fetch.

## Deterministic tests

Fixtures cover parallel arrays, catalog ranges, cross-file conflicts,
multi-company global budgets, canonical URLs and redaction.

## Live and load admission evidence

No operator-identified live/load admission has been recorded as of 2026-07-29,
so the formal trait remains unsupported.

## Explicit unsupported operations

Filing bodies, attachments, XBRL, archive crawling and unconfigured production
requests are unsupported.
