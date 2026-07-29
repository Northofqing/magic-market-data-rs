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

`FILING_METADATA_ADMITTED` remains `false`. Normal
`CompanyFilingsProvider::company_filings` calls therefore return
`Unsupported`. `probe_company_filings` and the examples exist only for
deterministic verification and the documented live admission procedure.

The live probe must pass twice and the serial load probe must pass before the
flag may be changed:

```bash
cargo run -p magic-sec-rs --example live_probe --offline
cargo run -p magic-sec-rs --example live_probe --offline
cargo run -p magic-sec-rs --example load_probe --offline
```

No live probe was run while implementing this crate.
