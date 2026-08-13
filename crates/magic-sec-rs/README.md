# magic-sec-rs

Official SEC EDGAR filing-metadata adapter for `data.sec.gov`.

The crate is intentionally metadata-only. It reads bounded submissions JSON
from these exact URL families:

- `https://data.sec.gov/submissions/CIK##########.json`
- `https://data.sec.gov/submissions/CIK##########-submissions-###.json`

Returned `www.sec.gov/Archives/` filing-index and primary-document URLs are
validated metadata. The adapter never fetches those URLs and does not expose
filing bodies, attachment lists, attachments, or XBRL facts.

## Fair access and identification

Production use requires a descriptive operator-supplied User-Agent:

```text
SEC_USER_AGENT="application/version contact@example.com"
```

The value is private and redacted from `Debug`. Requests are paced at one
start every 500 ms, redirects and cookies are disabled by the shared
transport, response bodies are limited to 8 MiB, status 403 is reported as an
identification failure, and status 429 remains an explicit HTTP failure. No
automatic retry occurs.

## Admission state

`FILING_METADATA_ADMITTED` is `true` for the bounded metadata-only contract.
`CompanyFilingsProvider::company_filings` and `probe_company_filings` share the
same strict implementation.

The admission evidence was produced by two live probes and one three-call
serial load probe:

```bash
cargo run -p magic-sec-rs --example live_probe --offline
cargo run -p magic-sec-rs --example live_probe --offline
cargo run -p magic-sec-rs --example load_probe --offline
```

The successful identified evidence was recorded on 2026-08-13 without storing
the operator-supplied User-Agent value.
