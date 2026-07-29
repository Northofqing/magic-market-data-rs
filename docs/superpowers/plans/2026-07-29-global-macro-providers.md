# Global Macro Provider Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan
> task-by-task. Follow red-green-refactor and never infer missing metadata.

**Goal:** Add FRED, IMF DataMapper v2, and World Bank Indicators v2 Providers
with exact source identities, bounded multi-series composition, explicit
authentication, missing-value semantics, and truthful admission.

**Architecture:** Each source has its own crate and shared bounded transport.
FRED performs one metadata plus one observations request per series and
requires a runtime API key. IMF validates the full bounded response because
the live v2 endpoint currently returns a superset despite path/period filters.
World Bank validates metadata and every data page atomically but remains
unadmitted while its structured unit field is empty under the approved
mandatory-unit Core contract.

**Tech Stack:** Rust 2021, Core `EconomicSeriesProvider`, shared transport,
`serde_json`, `time 0.3.54`, official HTTPS APIs only.

---

## Task 1: Scaffold source crates and configuration boundaries

**Files:**

- Modify: `Cargo.toml`
- Create: `crates/magic-fred-rs/Cargo.toml`
- Create: `crates/magic-fred-rs/src/lib.rs`
- Create: `crates/magic-fred-rs/tests/capabilities.rs`
- Create: `crates/magic-imf-rs/Cargo.toml`
- Create: `crates/magic-imf-rs/src/lib.rs`
- Create: `crates/magic-imf-rs/tests/capabilities.rs`
- Create: `crates/magic-worldbank-rs/Cargo.toml`
- Create: `crates/magic-worldbank-rs/src/lib.rs`
- Create: `crates/magic-worldbank-rs/tests/capabilities.rs`
- Modify: `Cargo.lock`

**Step 1: Register crates and manifests**

Use this manifest shape, changing the package name:

```toml
[package]
name = "magic-fred-rs"
version = "0.2.0"
edition.workspace = true
license.workspace = true

[dependencies]
magic-market-core = { path = "../magic-market-core", version = "=0.2.0" }
magic-market-transport = { path = "../magic-market-transport", version = "=0.2.0" }
serde = { workspace = true }
serde_json = "1"
thiserror = { workspace = true }
time = { version = "=0.3.54", default-features = false, features = ["formatting", "std"] }

[lints]
workspace = true
```

**Step 2: Write capability/configuration red tests**

FRED:

```rust
use magic_fred_rs::{FredClient, FredError};

#[test]
fn api_key_is_required_and_never_debugged() {
    assert!(matches!(FredClient::new(""), Err(FredError::Authentication(_))));
    let client = FredClient::new("secret-key-value").unwrap();
    assert!(!format!("{client:?}").contains("secret-key-value"));
}
```

IMF:

```rust
use magic_imf_rs::{ImfClient, ECONOMIC_SERIES_ADMITTED};

#[test]
fn capability_matches_live_admission_flag() {
    assert_eq!(
        ImfClient::economic_data_capabilities().economic_series,
        ECONOMIC_SERIES_ADMITTED
    );
}
```

World Bank:

```rust
use magic_worldbank_rs::{WorldBankClient, ECONOMIC_SERIES_ADMITTED};

#[test]
fn missing_structured_units_prevent_admission() {
    assert!(!ECONOMIC_SERIES_ADMITTED);
    assert!(!WorldBankClient::economic_data_capabilities().economic_series);
}
```

Run:

```bash
cargo test -p magic-fred-rs -p magic-imf-rs -p magic-worldbank-rs \
  --test capabilities --offline
```

Expected: unresolved clients and constants.

**Step 3: Add client/error shells**

Each error enum has `InvalidRequest`, `Authentication` where applicable,
transparent `Transport`, `Decode`, `Protocol`, `Unsupported`, and transparent
`Core`. Clients contain injected transport plus a 1-second shared gate.

FRED stores the key in a private redacted wrapper:

```rust
#[derive(Clone)]
struct ApiKey(String);

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ApiKey([REDACTED])")
    }
}
```

No public accessor returns the key. No error contains a request URL with query
values.

Start all admission flags false. Run capability tests. Expected: pass.

**Step 4: Commit**

```bash
cargo update --offline
cargo check -p magic-fred-rs -p magic-imf-rs -p magic-worldbank-rs --offline
git add Cargo.toml Cargo.lock crates/magic-fred-rs crates/magic-imf-rs \
  crates/magic-worldbank-rs
git commit -m "feat: scaffold global macro providers"
```

## Task 2: Implement authenticated FRED series retrieval

**Files:**

- Create: `crates/magic-fred-rs/src/parser.rs`
- Create: `crates/magic-fred-rs/src/transport.rs`
- Create: `crates/magic-fred-rs/tests/fixtures/series.json`
- Create: `crates/magic-fred-rs/tests/fixtures/observations.json`
- Create: `crates/magic-fred-rs/tests/parser.rs`
- Create: `crates/magic-fred-rs/examples/live_probe.rs`
- Create: `crates/magic-fred-rs/examples/load_probe.rs`
- Modify: `crates/magic-fred-rs/src/lib.rs`

**Step 1: Write metadata and missing-value red tests**

Synthetic metadata:

```json
{
  "seriess": [{
    "id":"GDP","title":"Gross Domestic Product",
    "observation_start":"1947-01-01","observation_end":"2026-04-01",
    "frequency":"Quarterly","frequency_short":"Q",
    "units":"Billions of Dollars","units_short":"Bil. of $",
    "seasonal_adjustment":"Seasonally Adjusted Annual Rate",
    "seasonal_adjustment_short":"SAAR",
    "last_updated":"2026-06-25 07:45:02-05",
    "notes":""
  }]
}
```

Synthetic observations:

```json
{
  "realtime_start":"2026-07-29","realtime_end":"2026-07-29",
  "observation_start":"2025-01-01","observation_end":"2025-12-31",
  "units":"lin","output_type":1,"file_type":"json",
  "order_by":"observation_date","sort_order":"asc",
  "count":4,"offset":0,"limit":100000,
  "observations":[
    {"realtime_start":"2026-07-29","realtime_end":"2026-07-29",
     "date":"2025-01-01","value":"30142.8"},
    {"realtime_start":"2026-07-29","realtime_end":"2026-07-29",
     "date":"2025-04-01","value":"."}
  ]
}
```

Tests prove:

- namespace is exactly `fred`;
- returned series ID, frequency, unit, and adjustment match metadata;
- `"."` maps to `Missing` with `None`;
- `"0"` maps to present zero;
- malformed/non-finite values fail;
- quarterly dates must be quarter starts;
- count/offset/limit and ascending order are consistent;
- metadata or observations from another series fail;
- API error envelopes map authentication separately without leaking the key.

Run:

```bash
cargo test -p magic-fred-rs --test parser --offline
```

Expected: unresolved parser.

**Step 2: Implement exact official requests**

Allow only:

```text
GET https://api.stlouisfed.org/fred/series
GET https://api.stlouisfed.org/fred/series/observations
```

Allowed query keys:

```text
api_key, file_type, series_id, observation_start, observation_end,
offset, limit, sort_order
```

Use JSON MIME, 4 MiB per response, timeout `1..=60s`, limit at most 100,000,
and no retry. Build the key only as a query value; transport diagnostics redact
it.

**Step 3: Implement atomic multi-series mapping**

Accept at most 20 FRED keys per call and require namespace `fred`. For each key:

1. fetch and validate exactly one metadata row;
2. fetch observations with date bounds derived from the requested checked
   period range;
3. require source frequency equal to request frequency;
4. validate every observation before filtering or truncating;
5. reject duplicate periods and any unit/adjustment change.

If any series fails, return no batch. Sort by requested series order and
period. Use metadata `last_updated` as optional source evidence only after
parsing it as a real timestamp; use realtime dates as revision labels, not
times.

**Step 4: Add probes and deterministic gates**

`live_probe` reads `FRED_API_KEY` and exits with typed authentication if absent.
It requests `GDP` for four quarters and prints no configuration. `load_probe`
performs exactly three serial calls.

Run:

```bash
cargo test -p magic-fred-rs --all-targets --offline
cargo clippy -p magic-fred-rs --all-targets --offline -- -D warnings
```

Expected: pass without an environment key because deterministic tests inject
transport.

**Step 5: Commit**

```bash
git add crates/magic-fred-rs
git commit -m "feat(fred): implement authenticated economic series"
```

## Task 3: Implement IMF DataMapper v2 with bounded superset validation

**Files:**

- Create: `crates/magic-imf-rs/src/parser.rs`
- Create: `crates/magic-imf-rs/src/transport.rs`
- Create: `crates/magic-imf-rs/tests/fixtures/indicators.json`
- Create: `crates/magic-imf-rs/tests/fixtures/series.json`
- Create: `crates/magic-imf-rs/tests/parser.rs`
- Create: `crates/magic-imf-rs/examples/live_probe.rs`
- Create: `crates/magic-imf-rs/examples/load_probe.rs`
- Modify: `crates/magic-imf-rs/src/lib.rs`

**Step 1: Fix provider-native key syntax with red tests**

Use namespace `DATASET/AREA`, for example `WEO/USA`, and code
`NGDP_RPCH`. Test:

```rust
#[test]
fn namespace_requires_dataset_and_area() {
    assert!(parse_namespace("WEO/USA").is_ok());
    assert!(parse_namespace("WEO").is_err());
    assert!(parse_namespace("WEO/USA/CHN").is_err());
    assert!(parse_namespace("../USA").is_err());
}
```

Only uppercase ASCII letters/digits/underscore/hyphen are accepted, each
component 1 through 32 characters.

**Step 2: Write catalog/series red tests**

The catalog fixture includes:

```json
{
  "indicators": {
    "NGDP_RPCH": {
      "label":"Real GDP growth",
      "source":"World Economic Outlook (April 2026)",
      "unit":"Annual percent change",
      "dataset":"WEO",
      "projection-year":2026,
      "last-modified":"2026-04-08 16:07:34"
    }
  }
}
```

The series fixture includes:

```json
{
  "api":{"version":"2","output-method":"json"},
  "indicators":{"NGDP_RPCH":{"label":"Real GDP growth"}},
  "values":{"NGDP_RPCH":{
    "USA":{"2024":2.8,"2025":2.0},
    "CHN":{"2024":5.0,"2025":4.5},
    "WEOWORLD":{"2024":3.4},
    "":null
  }}
}
```

Prove:

- API version/output method are exact;
- requested indicator exists in catalog and response;
- catalog dataset matches namespace;
- requested area exists;
- unrequested area/year supersets are fully shape-validated, then ignored;
- the single empty-key/null source sentinel is ignored, but any other null
  area map fails;
- negative and zero values remain present;
- non-finite, duplicate JSON keys, wrong unit, or wrong dataset fail;
- projected years receive a source-defined revision label derived from
  `projection-year`, not a fabricated status.

Run:

```bash
cargo test -p magic-imf-rs --test parser --offline
```

Expected: unresolved parser.

**Step 3: Implement the public v2 client**

Allow:

```text
GET https://www.imf.org/external/datamapper/api/v2/indicators
GET https://www.imf.org/external/datamapper/api/v2/{indicator}/{areas}
```

Allow only `periods` query. Cap 20 keys, 20 distinct areas, 50 requested years,
8 MiB catalog, 16 MiB series response, and 20,000 decoded area-year cells.
Since the audited live route ignored server-side filters, validate the complete
bounded response before selecting requested areas/years.

Build annual records with exact catalog unit and dataset. Use catalog
`last-modified` as source time only after parsing and assigning the explicitly
documented UTC interpretation in the integration doc; otherwise keep
`source_at=None` and retain the source string only as a revision label.

**Step 4: Add probes, pass, and commit**

The live probe requests `WEO/USA` and `WEO/CHN` `NGDP_RPCH` for 2024 and 2025.
The load probe makes three serial calls.

```bash
cargo test -p magic-imf-rs --all-targets --offline
cargo clippy -p magic-imf-rs --all-targets --offline -- -D warnings
git add crates/magic-imf-rs
git commit -m "feat(imf): implement bounded DataMapper series"
```

## Task 4: Implement World Bank v2 parser/client without inferred units

**Files:**

- Create: `crates/magic-worldbank-rs/src/parser.rs`
- Create: `crates/magic-worldbank-rs/src/transport.rs`
- Create: `crates/magic-worldbank-rs/tests/fixtures/indicator.json`
- Create: `crates/magic-worldbank-rs/tests/fixtures/data-page-1.json`
- Create: `crates/magic-worldbank-rs/tests/fixtures/data-page-2.json`
- Create: `crates/magic-worldbank-rs/tests/parser.rs`
- Create: `crates/magic-worldbank-rs/examples/live_probe.rs`
- Modify: `crates/magic-worldbank-rs/src/lib.rs`

**Step 1: Fix key syntax and write pagination red tests**

Use namespace grammar `source:SOURCE_ID/country:AREA_CODE`, for example
`source:2/country:USA`, and the indicator code as key code. Tests cover exact
parsing, source ID match, country/aggregate preservation, and unsafe
separators.

Use the official two-element JSON page fixture. Prove:

- page metadata `page`, `pages`, `per_page`, `total`, `sourceid`, and
  `lastupdated` is internally consistent;
- every row matches requested indicator and economy;
- country source ID/name and ISO-3 code are retained exactly;
- aggregates are not relabeled as countries;
- `null` maps to `Missing`, numeric zero to `Present`;
- page totals/source ID/last-updated/indicator metadata remain stable;
- any page failure, duplicate period, or conflicting value fails atomically;
- an empty structured indicator `unit` returns `Protocol`, not a guessed unit.

Run:

```bash
cargo test -p magic-worldbank-rs --test parser --offline
```

Expected: unresolved parser.

**Step 2: Implement exact v2 requests**

Allow:

```text
GET https://api.worldbank.org/v2/indicator/{indicator}
GET https://api.worldbank.org/v2/country/{economies}/indicator/{indicators}
```

Allowed queries are `format`, `date`, `page`, `per_page`, and `source`.
Require `format=json`, at most 60 indicators per official documentation,
`per_page <= 1_000`, `pages <= 100`, total rows `<= 10_000`, JSON MIME, and
8 MiB per page.

Fetch indicator metadata first, require non-empty structured `unit`, then fetch
all data pages. Never derive unit from indicator name, source note, or topic.

**Step 3: Expose diagnostic behavior and keep capability false**

`probe_economic_series` executes the exact production path and returns the
typed unit protocol error for the audited indicators. `economic_series`
returns `Unsupported` while `ECONOMIC_SERIES_ADMITTED=false`; it does not
return a fixture or partial rows.

The live probe requests `source:2/country:USA` and
`NY.GDP.MKTP.CD`, prints the typed missing-unit outcome, and exits successfully
only in an explicit diagnostic mode. It must not label this production
admission.

**Step 4: Pass and commit**

```bash
cargo test -p magic-worldbank-rs --all-targets --offline
cargo clippy -p magic-worldbank-rs --all-targets --offline -- -D warnings
git add crates/magic-worldbank-rs
git commit -m "feat(worldbank): add strict indicators diagnostic provider"
```

## Task 5: Run source-specific admission

**Files:**

- Modify: `crates/magic-fred-rs/src/lib.rs`
- Modify: `crates/magic-imf-rs/src/lib.rs`
- Create: `crates/magic-fred-rs/README.md`
- Create: `crates/magic-imf-rs/README.md`
- Create: `crates/magic-worldbank-rs/README.md`

**Step 1: FRED configured admission**

With `FRED_API_KEY` set only in the process environment:

```bash
cargo run -p magic-fred-rs --example live_probe --offline
cargo run -p magic-fred-rs --example live_probe --offline
cargo run -p magic-fred-rs --example load_probe --offline
```

If all three pass, set `ECONOMIC_SERIES_ADMITTED=true`. If the key is absent,
leave the flag false and document `configured/authentication required`.
Never place the key or a URL containing it in a file, shell trace, probe
output, or commit.

**Step 2: IMF anonymous admission**

```bash
cargo run -p magic-imf-rs --example live_probe --offline
cargo run -p magic-imf-rs --example live_probe --offline
cargo run -p magic-imf-rs --example load_probe --offline
```

Set the IMF flag true only if both full-envelope live validations and the
serial load probe pass within declared limits.

**Step 3: Preserve World Bank false admission**

Run the diagnostic once:

```bash
cargo run -p magic-worldbank-rs --example live_probe --offline
```

Record the exact structured-unit failure. Leave
`ECONOMIC_SERIES_ADMITTED=false`; changing the mandatory-unit contract or
adding another official unit source requires a separate reviewed design.

**Step 4: Package checkpoint and commit**

```bash
cargo fmt --all -- --check
cargo test -p magic-fred-rs -p magic-imf-rs -p magic-worldbank-rs \
  --all-targets --offline
cargo clippy -p magic-fred-rs -p magic-imf-rs -p magic-worldbank-rs \
  --all-targets --offline -- -D warnings
git diff --check
git add crates/magic-fred-rs crates/magic-imf-rs crates/magic-worldbank-rs
git commit -m "docs: record global macro provider admission"
```
