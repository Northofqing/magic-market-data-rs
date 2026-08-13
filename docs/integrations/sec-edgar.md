# SEC EDGAR integration

## Capability state

Filing metadata admission is true. Filing documents and XBRL facts are not
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
filters; catalog ranges must be ordered and non-overlapping. The accession's
first ten digits identify the submitting login CIK and may differ from the
subject-company CIK, including when a filing agent submits on its behalf; both
identities remain unmodified and the canonical archive path stays rooted under
the subject company's CIK. Primary-document identity accepts the bounded safe
relative subpaths emitted by SEC XML/Online Forms submissions while rejecting
absolute paths, empty or traversal segments, backslashes, controls and encoded
path separators.
Document extensions are source metadata rather than an allowlist: SEC currently
emits HTML, XML, text, PDF and legacy `.paper` identities. This client does not
fetch any of them; only the official submissions JSON is transported.

## Authentication or usage-rights boundary

`SEC_USER_AGENT` must be descriptive and is always redacted. Status 403 is an
identification failure; 429 remains explicit. There is no retry, login or
attachment/body fetch.

## Deterministic tests

Fixtures cover parallel arrays, catalog ranges, cross-file conflicts,
multi-company global budgets, canonical URLs and redaction.

## Live and load admission evidence

On 2026-08-13, two independent identified live runs each returned five current
Apple filing records from the official submissions API, including a filing
submitted through a distinct login/agent CIK. A three-call serial load probe
then proved one in-flight request and at least 500 ms between starts. The
operator contact remained process-local and was not written into evidence.
The formal metadata-only trait is admitted under these exact bounds.

## Explicit unsupported operations

Filing bodies, attachments, XBRL, archive crawling and unconfigured production
requests are unsupported.
