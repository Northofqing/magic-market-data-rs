# SEC EDGAR Filing Metadata Provider Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan
> task-by-task. Follow red-green-refactor and retain metadata-only scope.

**Goal:** Add an official SEC EDGAR Provider that atomically retrieves recent
and referenced older submission metadata, validates exact CIK/accession/path
identity, and returns canonical filing links without downloading filing bodies
or attachments.

**Architecture:** `magic-sec-rs` uses `data.sec.gov` submissions endpoints with
an operator-supplied descriptive User-Agent and a conservative clone-shared
request-start gate. It creates but does not fetch canonical `www.sec.gov`
archive URLs. Core owns checked CIK/accession/document types; the Provider owns
SEC host/path and parallel-array protocol validation.

**Tech Stack:** Rust 2021, Core filing contracts, shared transport,
`serde_json`, `time 0.3.54`, official SEC public data endpoints.

---

## Task 1: Scaffold the SEC crate and configuration

**Files:**

- Modify: `Cargo.toml`
- Create: `crates/magic-sec-rs/Cargo.toml`
- Create: `crates/magic-sec-rs/src/lib.rs`
- Create: `crates/magic-sec-rs/tests/capabilities.rs`
- Modify: `Cargo.lock`

**Step 1: Register the crate and manifest**

```toml
[package]
name = "magic-sec-rs"
version = "0.2.0"
edition.workspace = true
license.workspace = true

[dependencies]
magic-market-core = { path = "../magic-market-core", version = "=0.2.0" }
magic-market-transport = { path = "../magic-market-transport", version = "=0.2.0" }
serde = { workspace = true }
serde_json = "1"
thiserror = { workspace = true }
time = { version = "=0.3.54", default-features = false, features = ["formatting", "parsing", "std"] }

[lints]
workspace = true
```

**Step 2: Write configuration red tests**

```rust
use magic_sec_rs::{SecEdgarClient, SecEdgarError};

#[test]
fn descriptive_user_agent_is_required_and_redacted() {
    assert!(matches!(
        SecEdgarClient::new(""),
        Err(SecEdgarError::InvalidRequest(_))
    ));
    assert!(matches!(
        SecEdgarClient::new("anonymous-client"),
        Err(SecEdgarError::InvalidRequest(_))
    ));
    let client = SecEdgarClient::new(
        "magic-market-data-rs/0.2 operations@example.com",
    ).unwrap();
    let debug = format!("{client:?}");
    assert!(!debug.contains("operations@example.com"));
    assert!(debug.contains("[REDACTED]"));
}
```

Run:

```bash
cargo test -p magic-sec-rs --test capabilities --offline
```

Expected: unresolved client/error.

**Step 3: Implement client shell and error categories**

Validate User-Agent length `10..=256`, no controls, at least one ASCII
application token containing `/`, and one contact token containing `@` with
characters on both sides. Store it in a private wrapper whose `Debug` is
`SecUserAgent([REDACTED])`.

Use:

```rust
#[derive(Debug, Error)]
pub enum SecEdgarError {
    #[error("invalid SEC request: {0}")]
    InvalidRequest(String),
    #[error("SEC authentication/identification failed: {0}")]
    Authentication(String),
    #[error(transparent)]
    Transport(#[from] magic_market_transport::TransportError),
    #[error("SEC response decoding failed: {0}")]
    Decode(String),
    #[error("SEC submissions protocol failed: {0}")]
    Protocol(String),
    #[error("unsupported SEC capability: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Core(#[from] magic_market_core::CoreError),
}
```

Start `FILING_METADATA_ADMITTED=false`; capabilities always keep documents and
XBRL false.

**Step 4: Commit**

```bash
cargo update --offline
cargo check -p magic-sec-rs --offline
git add Cargo.toml Cargo.lock crates/magic-sec-rs
git commit -m "feat(sec): scaffold EDGAR metadata provider"
```

## Task 2: Parse recent submission metadata and construct canonical URLs

**Files:**

- Create: `crates/magic-sec-rs/src/parser.rs`
- Create: `crates/magic-sec-rs/src/transport.rs`
- Create: `crates/magic-sec-rs/tests/fixtures/submissions.json`
- Create: `crates/magic-sec-rs/tests/parser.rs`
- Modify: `crates/magic-sec-rs/src/lib.rs`

**Step 1: Write the exact recent-submissions fixture**

```json
{
  "cik":"0000320193",
  "entityType":"operating",
  "sic":"3571",
  "sicDescription":"Electronic Computers",
  "name":"Apple Inc.",
  "tickers":["AAPL"],
  "exchanges":["Nasdaq"],
  "filings":{
    "recent":{
      "accessionNumber":["0000320193-25-000079","0000320193-25-000057"],
      "filingDate":["2025-05-02","2025-04-04"],
      "reportDate":["2025-03-29",""],
      "acceptanceDateTime":["2025-05-01T18:26:29.000Z","2025-04-04T16:31:42.000Z"],
      "act":["34","34"],
      "form":["10-Q","8-K"],
      "fileNumber":["001-36743","001-36743"],
      "filmNumber":["25905524","25712642"],
      "items":["","5.07"],
      "size":[15234567,412345],
      "isXBRL":[1,1],
      "isInlineXBRL":[1,1],
      "primaryDocument":["aapl-20250329.htm","aapl-20250404.htm"],
      "primaryDocDescription":["10-Q","8-K"]
    },
    "files":[
      {"name":"CIK0000320193-submissions-001.json",
       "filingCount":2000,"filingFrom":"2015-01-01","filingTo":"2024-12-31"}
    ]
  }
}
```

**Step 2: Write red tests for parallel-array invariants**

Prove:

- response CIK equals the requested normalized CIK;
- company name is non-empty;
- ticker is retained only when supplied by the response and matches the
  requested optional ticker case-insensitively;
- every required recent array has exactly the same length;
- blank report date maps to `None`;
- accession CIK prefix matches response CIK;
- forms and dates filter after complete array validation;
- source order is newest filing date then acceptance time;
- malformed acceptance timestamps fail rather than becoming source time;
- unsafe primary document names fail;
- duplicate CIK+accession rows collapse only when every field is equal;
- conflicting duplicates fail the complete request.

Run:

```bash
cargo test -p magic-sec-rs --test parser --offline
```

Expected: unresolved parser.

**Step 3: Implement bounded parsing**

Deserialize required recent fields into vectors and validate equal lengths
before indexing. Limits:

```rust
const MAX_SUBMISSIONS_BYTES: usize = 8 * 1024 * 1024;
const MAX_RECENT_FILINGS: usize = 2_000;
const MAX_OLDER_FILES: usize = 20;
const MAX_COMPANY_NAME_CHARS: usize = 512;
```

Construct URLs using the normalized CIK without leading zeros for the archive
directory:

```rust
let archive_cik = company.cik().trim_start_matches('0');
let archive_cik = if archive_cik.is_empty() { "0" } else { archive_cik };
let compact = accession.without_hyphens();
let index = format!(
    "https://www.sec.gov/Archives/edgar/data/{archive_cik}/{compact}/{}-index.html",
    accession.as_str(),
);
let primary = format!(
    "https://www.sec.gov/Archives/edgar/data/{archive_cik}/{compact}/{}",
    primary_document.as_str(),
);
```

Validate both generated URLs again: HTTPS, host exactly `www.sec.gov`, port
443/default, archive prefix exact, no query/fragment/credentials.

Use acceptance time for record `source_at`; use the maximum accepted time as
batch `source_at`. If a row has no accepted time, retain it with no source time
but do not substitute fetch time.

**Step 4: Implement exact submissions GET**

Allow only:

```text
GET https://data.sec.gov/submissions/CIK##########.json
```

No query keys, JSON MIME, 8 MiB, timeout `1..=60s`, no cookies, no redirects.
Send `User-Agent` and `Accept-Encoding: identity`. The shared request interval
is 500 ms, below SEC's published maximum rate; no automatic retry occurs.
Status 403 maps `Authentication`, 429 remains an explicit transport HTTP
failure.

**Step 5: Pass and commit**

```bash
cargo test -p magic-sec-rs --test parser --offline
cargo clippy -p magic-sec-rs --all-targets --offline -- -D warnings
git add crates/magic-sec-rs
git commit -m "feat(sec): parse recent EDGAR filing metadata"
```

## Task 3: Add older-submission pagination atomically

**Files:**

- Create: `crates/magic-sec-rs/tests/fixtures/submissions-older.json`
- Create: `crates/magic-sec-rs/tests/pagination.rs`
- Modify: `crates/magic-sec-rs/src/parser.rs`
- Modify: `crates/magic-sec-rs/src/transport.rs`
- Modify: `crates/magic-sec-rs/src/lib.rs`

**Step 1: Write older-file red tests**

The older file fixture has the same filing arrays at its root, without company
metadata. Test:

- referenced filename is exactly
  `CIK##########-submissions-###.json`;
- filename CIK matches the parent;
- catalog filing count/range is checked against decoded rows;
- only older files intersecting the requested date range are fetched;
- a request with no date range fetches older files only until
  `max_records` can be satisfied, but validates every fetched file fully;
- file ranges must not overlap contradictory recent/older data;
- any referenced-file failure makes the complete company request fail;
- multiple companies compose atomically and sort by requested company order,
  filing date descending, acceptance time descending;
- output truncation occurs only after all required files are validated.

Run:

```bash
cargo test -p magic-sec-rs --test pagination --offline
```

Expected: older pagination assertions fail.

**Step 2: Implement the referenced-file policy**

Extend the exact allowlist to:

```text
/submissions/CIK##########.json
/submissions/CIK##########-submissions-###.json
```

Reject arbitrary filenames before I/O. Fetch at most 20 older files and 20,000
decoded filing rows per company. Reuse the same parallel-array parser. Compare
parent descriptor `filingFrom`/`filingTo` to the actual min/max filing dates;
reject contradictory metadata.

**Step 3: Implement `CompanyFilingsProvider`**

Before I/O require every request company CIK valid, at most 100 companies,
form/date filters valid, and a practical Provider cap of 10 companies per one
call. Return `InvalidRequest` for 11 rather than silently truncating.

Process companies in request order. If one fails, return the typed error and no
partial `DataBatch`. Business identity is `(cik, accession)`.

**Step 4: Pass and commit**

```bash
cargo test -p magic-sec-rs --all-targets --offline
cargo clippy -p magic-sec-rs --all-targets --offline -- -D warnings
git add crates/magic-sec-rs
git commit -m "feat(sec): add atomic older filing pagination"
```

## Task 4: Add metadata-only live/load probes and admission

**Files:**

- Create: `crates/magic-sec-rs/examples/live_probe.rs`
- Create: `crates/magic-sec-rs/examples/load_probe.rs`
- Create: `crates/magic-sec-rs/README.md`
- Modify: `crates/magic-sec-rs/src/lib.rs`

**Step 1: Implement bounded probes**

`live_probe` requires:

```text
SEC_USER_AGENT="application/version contact@example.com"
```

It requests one public CIK, at most five records, forms `10-K`, `10-Q`, and
`8-K`, and prints only:

```text
CIK ticker company form filing_date report_period accession
filing_index_url primary_document_url accepted_at observed_at batch_id
```

It does not fetch either URL. `load_probe` performs exactly three serial
submissions requests and verifies maximum concurrency 1 and at least 500 ms
request-start spacing.

**Step 2: Prove metadata-only behavior**

Add an integration test with a transport that records requests and assert that
all calls target `data.sec.gov/submissions/`; no call targets
`www.sec.gov/Archives/`. Assert the normalized record has no body, attachment,
XBRL fact, or attachment-list field.

**Step 3: Run admission**

```bash
cargo run -p magic-sec-rs --example live_probe --offline
cargo run -p magic-sec-rs --example live_probe --offline
cargo run -p magic-sec-rs --example load_probe --offline
```

Run with network access and the environment variable set. If both live
production runs and the load probe pass, set:

```rust
pub const FILING_METADATA_ADMITTED: bool = true;
```

Otherwise leave it false and record the exact typed failure. Document SEC fair
access, User-Agent configuration, host/path bounds, 500 ms pacing, 429
behavior, and metadata-only scope.

**Step 4: Checkpoint and commit**

```bash
cargo fmt --all -- --check
cargo test -p magic-sec-rs --all-targets --offline
cargo clippy -p magic-sec-rs --all-targets --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p magic-sec-rs --no-deps --offline
git diff --check
git add crates/magic-sec-rs
git commit -m "docs: record SEC EDGAR metadata admission"
```
