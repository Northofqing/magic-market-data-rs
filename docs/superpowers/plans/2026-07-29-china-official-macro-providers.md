# China Official Macro Provider Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan
> task-by-task. Follow red-green-refactor and preserve unsupported states.

**Goal:** Add source-aligned NBS, PBC, and CFETS crates with strict official
source identity, bounded requests, exact missing/unit semantics, and truthful
capability admission.

**Architecture:** All three crates use `magic-market-transport`; none depends
on another Provider. NBS ships a deterministic diagnostic parser and keeps its
production capability false because the source audit did not prove a supported
machine contract. PBC initially admits only cataloged official HTML
money-supply tables. CFETS initially admits bounded Shibor, LPR, and official
central-parity histories; DR007 stays false because no equivalent official
history contract was proven.

**Tech Stack:** Rust 2021, Core macro/reference contracts, shared transport,
`serde_json`, `time 0.3.54`, strict marker/table parsing without a DOM, official
PBC and China Money HTTPS endpoints.

---

## Task 1: Scaffold the three source crates with truthful capabilities

**Files:**

- Modify: `Cargo.toml`
- Create: `crates/magic-nbs-rs/Cargo.toml`
- Create: `crates/magic-nbs-rs/src/lib.rs`
- Create: `crates/magic-nbs-rs/tests/capabilities.rs`
- Create: `crates/magic-pbc-rs/Cargo.toml`
- Create: `crates/magic-pbc-rs/src/lib.rs`
- Create: `crates/magic-pbc-rs/tests/capabilities.rs`
- Create: `crates/magic-cfets-rs/Cargo.toml`
- Create: `crates/magic-cfets-rs/src/lib.rs`
- Create: `crates/magic-cfets-rs/tests/capabilities.rs`
- Modify: `Cargo.lock`

**Step 1: Register workspace members and common manifests**

Add the three crate paths to root `members`. Each manifest uses:

```toml
[package]
name = "magic-nbs-rs"
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

Use the corresponding package name for PBC and CFETS. No crate adds a browser,
PDF, spreadsheet, or cookie dependency.

**Step 2: Write capability red tests**

NBS:

```rust
use magic_nbs_rs::{NbsClient, NATIONAL_SERIES_ADMITTED, REGIONAL_SERIES_ADMITTED};

#[test]
fn unproved_machine_contract_is_not_advertised() {
    let capabilities = NbsClient::economic_data_capabilities();
    assert!(!NATIONAL_SERIES_ADMITTED);
    assert!(!REGIONAL_SERIES_ADMITTED);
    assert!(!capabilities.economic_series);
    assert!(!capabilities.regional_series);
}
```

PBC:

```rust
use magic_pbc_rs::{
    PbcClient, MONEY_SUPPLY_ADMITTED, REGIONAL_SERIES_ADMITTED,
    SOCIAL_FINANCING_ADMITTED,
};

#[test]
fn only_the_audited_table_family_can_be_admitted() {
    let capabilities = PbcClient::economic_data_capabilities();
    assert_eq!(capabilities.economic_series, MONEY_SUPPLY_ADMITTED);
    assert!(!SOCIAL_FINANCING_ADMITTED);
    assert!(!REGIONAL_SERIES_ADMITTED);
}
```

CFETS:

```rust
use magic_cfets_rs::{
    CfetsClient, DR007_ADMITTED, LPR_ADMITTED, OFFICIAL_FX_ADMITTED,
    SHIBOR_ADMITTED,
};

#[test]
fn capabilities_are_family_specific() {
    let source = CfetsClient::capabilities();
    assert_eq!(source.shibor, SHIBOR_ADMITTED);
    assert_eq!(source.loan_prime_rate, LPR_ADMITTED);
    assert!(!source.dr007);
    assert_eq!(source.official_fx_fixings, OFFICIAL_FX_ADMITTED);
    let core = CfetsClient::reference_data_capabilities();
    assert_eq!(
        core.benchmark_rates,
        SHIBOR_ADMITTED || LPR_ADMITTED || DR007_ADMITTED
    );
    assert_eq!(core.official_fx_fixings, OFFICIAL_FX_ADMITTED);
}
```

Run:

```bash
cargo test -p magic-nbs-rs -p magic-pbc-rs -p magic-cfets-rs \
  --test capabilities --offline
```

Expected: unresolved client/constants.

**Step 3: Add client shells and typed errors**

Each client is cloneable, contains `Arc<dyn HttpTransport>` and
`Arc<RequestGate>`, exposes an injected-transport constructor, and uses typed
errors:

```rust
#[derive(Debug, Error)]
pub enum PbcError {
    #[error("invalid PBC request: {0}")]
    InvalidRequest(String),
    #[error(transparent)]
    Transport(#[from] magic_market_transport::TransportError),
    #[error("PBC response decoding failed: {0}")]
    Decode(String),
    #[error("PBC source contract failed: {0}")]
    Protocol(String),
    #[error("unsupported PBC capability: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Core(#[from] magic_market_core::CoreError),
}
```

NBS and CFETS use the same categories with their names. Set:

```rust
pub const NATIONAL_SERIES_ADMITTED: bool = false; // NBS
pub const REGIONAL_SERIES_ADMITTED: bool = false; // NBS and PBC
pub const SOCIAL_FINANCING_ADMITTED: bool = false; // PBC
pub const DR007_ADMITTED: bool = false; // CFETS
```

PBC's `MONEY_SUPPLY_ADMITTED`, and CFETS's Shibor/LPR/fixing flags also start
false. Add a source-specific `CfetsCapabilities` with `shibor`,
`loan_prime_rate`, `dr007`, and `official_fx_fixings`; the Core
`ReferenceDataCapabilities` collapses those to generic `benchmark_rates` and
`official_fx_fixings`. Flags change only in the admission task after the
required live evidence passes. Run the capability tests. Expected: pass.

**Step 4: Commit the scaffold**

```bash
cargo update --offline
cargo check -p magic-nbs-rs -p magic-pbc-rs -p magic-cfets-rs --offline
git add Cargo.toml Cargo.lock crates/magic-nbs-rs crates/magic-pbc-rs \
  crates/magic-cfets-rs
git commit -m "feat: scaffold China official data providers"
```

## Task 2: Implement the NBS diagnostic boundary without false admission

**Files:**

- Create: `crates/magic-nbs-rs/src/parser.rs`
- Create: `crates/magic-nbs-rs/src/transport.rs`
- Create: `crates/magic-nbs-rs/tests/fixtures/national-monthly.json`
- Create: `crates/magic-nbs-rs/tests/parser.rs`
- Create: `crates/magic-nbs-rs/examples/live_probe.rs`
- Modify: `crates/magic-nbs-rs/src/lib.rs`

**Step 1: Write strict parser fixture tests**

Use a synthetic fixture containing:

```json
{
  "returncode": 200,
  "returndata": {
    "wdnodes": [
      {"wdcode": "zb", "nodes": [{"code": "A010101", "name": "指标甲", "unit": "点"}]},
      {"wdcode": "sj", "nodes": [{"code": "202506", "name": "2025年6月"}]}
    ],
    "datanodes": [
      {"code": "zb.A010101_sj.202506", "data": {"data": 0.0, "hasdata": true}}
    ]
  }
}
```

Test that zero with `hasdata=true` becomes `Present`, `hasdata=false` with no
numeric value becomes `Missing`, a returned `zb` or `sj` not requested fails,
duplicate node identities fail, absent unit fails, malformed month fails, and
records use `ProviderId::Nbs`.

Run:

```bash
cargo test -p magic-nbs-rs --test parser --offline
```

Expected: unresolved parser.

**Step 2: Implement a bounded diagnostic parser**

`parse_national_monthly_fixture` accepts:

```rust
pub(crate) fn parse_national_monthly_fixture(
    body: &[u8],
    request: &EconomicSeriesRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<DataBatch<EconomicObservation>, NbsError>
```

Reject bodies over 4 MiB before `serde_json::from_slice`. Deserialize only
`returncode`, `wdnodes`, and `datanodes`; cap metadata nodes at 1,000 and data
nodes at 10,000. Require `returncode == 200`, exact requested namespace
`national-monthly`, exact `zb` codes, range-bounded `sj` months, one unit per
series, and no unrequested rows. Apply the request row ceiling only after
validating the complete fixture.

**Step 3: Implement explicit diagnostics and unsupported production**

Expose a `NbsDiagnosticRequest` containing an exact caller-supplied public
response body plus the normalized request. It is fixture/offline diagnostics,
not network fallback:

```rust
pub fn probe_national_payload(
    &self,
    request: &EconomicSeriesRequest,
    body: &[u8],
    observed_at: &str,
) -> Result<DataBatch<EconomicObservation>, NbsError>
```

Implement `EconomicSeriesProvider` as:

```rust
fn economic_series(
    &self,
    _request: &EconomicSeriesRequest,
) -> Result<DataBatch<EconomicObservation>, Self::Error> {
    Err(NbsError::Unsupported(
        "NBS production access is not admitted: the official site exposes no supported machine contract and rejected the audited minimal client"
            .into(),
    ))
}
```

`live_probe` performs only a bounded GET of the documented public landing page
and reports HTTP/transport status; it does not replay hidden AJAX calls or
solve anti-bot challenges.

**Step 4: Pass and commit**

```bash
cargo test -p magic-nbs-rs --all-targets --offline
cargo clippy -p magic-nbs-rs --all-targets --offline -- -D warnings
git add crates/magic-nbs-rs
git commit -m "feat(nbs): add strict diagnostic parser boundary"
```

## Task 3: Implement PBC official HTML money-supply tables

**Files:**

- Create: `crates/magic-pbc-rs/src/catalog.rs`
- Create: `crates/magic-pbc-rs/src/html.rs`
- Create: `crates/magic-pbc-rs/src/transport.rs`
- Create: `crates/magic-pbc-rs/tests/fixtures/money-supply-2024.html`
- Create: `crates/magic-pbc-rs/tests/fixtures/money-supply-revision.html`
- Create: `crates/magic-pbc-rs/tests/html.rs`
- Create: `crates/magic-pbc-rs/examples/live_probe.rs`
- Create: `crates/magic-pbc-rs/examples/load_probe.rs`
- Modify: `crates/magic-pbc-rs/src/lib.rs`

**Step 1: Define the exact catalog**

Use a checked descriptor:

```rust
pub struct PbcTableDescriptor {
    year: u16,
    namespace: &'static str,
    canonical_url: &'static str,
    title_zh: &'static str,
    title_en: &'static str,
    unit_zh: &'static str,
    unit_en: &'static str,
}
```

Add only URLs individually proven as official structured HTML. The initial
catalog includes:

```rust
PbcTableDescriptor {
    year: 2024,
    namespace: "money-supply",
    canonical_url:
        "https://www.pbc.gov.cn/eportal/fileDir/diaochatongjisi/resource/cms/2024/11/2024111416041159339.htm",
    title_zh: "货币供应量",
    title_en: "Money Supply",
    unit_zh: "亿元人民币",
    unit_en: "100 Million Yuan",
}
```

Do not infer URL IDs for another year. A request for an uncataloged year fails
with `Unsupported` before transport.

**Step 2: Write table red tests**

The 2024 synthetic fixture must include title, bilingual unit, twelve month
headers, and M0/M1/M2 rows. A separate synthetic revision fixture uses 2025
periods and the exact January-2025 M1 methodology-break shape. Tests prove:

- `M0`, `M1`, and `M2` row labels map only by exact bilingual aliases;
- `0` is a present value;
- blank/`—` becomes `Missing`, never zero;
- every month header is unique and `YYYY.MM`;
- the cataloged November 2024 table's blank November/December cells map to
  explicit `Missing` rows only when those periods are requested;
- a title, unit, header-count, row-label, or footnote mutation fails;
- requested subset/range is applied only after validating all table rows;
- the M1 revision note maps to `EconomicRevisionKind::SourceDefined`;
- `summary`-style surrounding article text is never parsed as data.

Run:

```bash
cargo test -p magic-pbc-rs --test html --offline
```

Expected: unresolved parser/catalog.

**Step 3: Implement strict table scanning**

Implement a small bounded tokenizer over table tags (`table`, `caption`, `tr`,
`th`, `td`, `sup`, `p`) that decodes the five named HTML entities used by the
fixture and rejects malformed nesting. Support only positive bounded
`rowspan`/`colspan` attributes and expand them into a rectangular occupancy
grid; reject overlap, overflow, unknown span syntax, or any other layout that
cannot reproduce the audited three-level M2/M1/M0 label hierarchy. Do not use
regex to parse the table.
Limits:

```rust
const MAX_HTML_BYTES: usize = 2 * 1024 * 1024;
const MAX_ROWS: usize = 100;
const MAX_COLUMNS: usize = 14;
const MAX_CELL_CHARS: usize = 512;
```

`parse_money_supply_table` requires exact descriptor title/unit facts and
constructs `EconomicSeriesKey::new(ProviderId::Pbc, "money-supply", code)`.
Locate the exact primary month-header row and the next three balance rows;
validate the separate footnote/growth subtable without mapping its percentage
cells as money-supply balances. Use period months, source unit `亿元人民币`,
scale `100 million yuan`, and the applicable source footnote as the revision
label. Because the table does not prove an exact release timestamp, record and
batch `source_at` stay absent.

**Step 4: Wire the production client**

Build exact GET requests only for catalog URLs with policy:

```rust
EndpointPolicy::new(
    "www.pbc.gov.cn",
    vec!["/eportal/fileDir/diaochatongjisi/resource/cms/".into()],
    vec![],
    vec![MediaType::Html],
    MAX_HTML_BYTES,
    timeout,
)
```

The client allows 1 through 3 requested codes, monthly ranges, and cataloged
years. It fetches every required annual table atomically, validates each full
table, rejects overlapping conflicting months, sorts by request code then
month, and uses a 1-second shared gate.

`social-financing` and every other namespace return typed `Unsupported` before
I/O. No PDF or XLSX fallback occurs.

**Step 5: Add deterministic probes**

`live_probe` requests one cataloged table, prints series code, period, status,
value, unit, revision label, observed time, and batch ID. `load_probe` performs
exactly three serial calls at the configured interval and uses
`verify_serial_load`.

Run:

```bash
cargo test -p magic-pbc-rs --all-targets --offline
cargo clippy -p magic-pbc-rs --all-targets --offline -- -D warnings
```

Expected: pass.

**Step 6: Commit**

Keep `MONEY_SUPPLY_ADMITTED=false` until Task 5. Commit:

```bash
git add crates/magic-pbc-rs
git commit -m "feat(pbc): implement official money supply tables"
```

## Task 4: Implement CFETS Shibor, LPR, and central-parity histories

**Files:**

- Create: `crates/magic-cfets-rs/src/rates.rs`
- Create: `crates/magic-cfets-rs/src/fx.rs`
- Create: `crates/magic-cfets-rs/src/transport.rs`
- Create: `crates/magic-cfets-rs/tests/fixtures/shibor.json`
- Create: `crates/magic-cfets-rs/tests/fixtures/lpr.json`
- Create: `crates/magic-cfets-rs/tests/fixtures/ccpr-page-1.json`
- Create: `crates/magic-cfets-rs/tests/fixtures/ccpr-page-2.json`
- Create: `crates/magic-cfets-rs/tests/rates.rs`
- Create: `crates/magic-cfets-rs/tests/fx.rs`
- Create: `crates/magic-cfets-rs/examples/live_probe.rs`
- Create: `crates/magic-cfets-rs/examples/load_probe.rs`
- Modify: `crates/magic-cfets-rs/src/lib.rs`

**Step 1: Write Shibor/LPR red tests**

Use synthetic fixtures matching the audited envelopes:

```json
{
  "data": {
    "baseCurveCfgList": [
      {"cfgItem":"O/N","cfgItemNm":"ON","sqncCd":1},
      {"cfgItem":"1W","cfgItemNm":"1W","sqncCd":2},
      {"cfgItem":"2W","cfgItemNm":"2W","sqncCd":3},
      {"cfgItem":"1M","cfgItemNm":"1M","sqncCd":4},
      {"cfgItem":"3M","cfgItemNm":"3M","sqncCd":5},
      {"cfgItem":"6M","cfgItemNm":"6M","sqncCd":6},
      {"cfgItem":"9M","cfgItemNm":"9M","sqncCd":7},
      {"cfgItem":"1Y","cfgItemNm":"1Y","sqncCd":8}
    ],
    "startDateCN":"2026-07-28",
    "endDateCN":"2026-07-29",
    "message":""
  },
  "records":[
    {"showDateCN":"2026-07-29","ON":"1.4150","1W":"1.4600",
     "2W":"1.4500","1M":"1.4200","3M":"1.4300","6M":"1.4505",
     "9M":"1.4700","1Y":"1.4800"}
  ]
}
```

Tests prove exact eight-tenor ordering, LPR's exact `1Y`/`5Y` headings,
percent units, requested identity matching, finite values, duplicate dates,
blank values, message errors, date bounds, and full-record validation before
output truncation.

Run:

```bash
cargo test -p magic-cfets-rs --test rates --offline
```

Expected: unresolved parser.

**Step 2: Implement rate request mapping**

Use exact routes:

```text
POST /ags/ms/cm-u-bk-shibor/ShiborHis?lang=cn&startDate=YYYY-MM-DD&endDate=YYYY-MM-DD
POST /ags/ms/cm-u-bk-currency/LprHis?lang=CN&strStartDate=YYYY-MM-DD&strEndDate=YYYY-MM-DD
```

Allow query keys `lang`, `startDate`, `endDate`, `strStartDate`, and
`strEndDate`, response MIME JSON, 2 MiB ceiling, timeout `1..=60s`, and a
1-second request-start gate. Map Shibor and LPR rates with
`RatioUnit::Percent`. The source pages document scheduled publication times but
the history response does not carry an unambiguous per-row timestamp, so
`published_at` and evidence `source_at` stay absent.

Any `Dr007` request returns:

```rust
Err(CfetsError::Unsupported(
    "CFETS DR007 history has no separately proven public contract".into(),
))
```

before I/O.

**Step 3: Write central-parity red tests**

Fixture fields:

```json
{
  "data": {
    "head": ["USD/CNY", "EUR/CNY", "100JPY/CNY", "CNY/KRW"],
    "total": 2, "pageTotal": 2, "pageSize": 1, "pageNum": 1,
    "currency": "USD/CNY,100JPY/CNY,CNY/KRW",
    "searchlist": ["USD/CNY", "100JPY/CNY", "CNY/KRW"],
    "startDate": "2026-07-28", "endDate": "2026-07-29",
    "flagMessage": ""
  },
  "records": [{"date":"2026-07-29","values":["6.7928","4.5660","193.72"]}]
}
```

Prove:

- headings parse as `USD/CNY` base 1, `100JPY/CNY` base 100, and `CNY/KRW`
  base 1 without reversing pairs;
- `data.head` is validated as the complete supported-currency catalog, while
  `data.currency` and `data.searchlist` must both equal the requested selected
  order; positional `values` are zipped only with that selected order;
- positive finite values are required;
- the positional value count equals the selected-currency count;
- page number, page count, total, source bounds, headings, and currency set
  stay stable across pages;
- any page failure makes the complete request fail;
- duplicate dates or conflicting values fail.

Run:

```bash
cargo test -p magic-cfets-rs --test fx --offline
```

Expected: unresolved parser.

**Step 4: Implement atomic central-parity pagination**

Use:

```text
GET /ags/ms/cm-u-bk-ccpr/CcprHisNew
  ?startDate=YYYY-MM-DD
  &endDate=YYYY-MM-DD
  &currency=comma-separated-source-heading
  &pageNum=N
  &pageSize=50
```

Map request identities to exact source headings through a closed table. Include
all 25 audited headings with their exact orientation and quotation base. Do
not infer quotation base from a currency name outside that table. Fetch at
most 20 pages and 1,000 rows; parse and validate all pages before constructing
the batch.

**Step 5: Add probes and pass deterministic gates**

The bounded live probe requests:

- Shibor overnight and one-week for the latest two business dates supplied on
  the command line;
- LPR one-year and over-five-year for a caller-supplied month;
- USD/CNY and 100JPY/CNY for the same two dates.

It prints exact source identities, fixing dates, values, units/quotation bases,
observed times, and batch IDs. The load probe performs exactly three serial
requests per admitted family.

Run:

```bash
cargo test -p magic-cfets-rs --all-targets --offline
cargo clippy -p magic-cfets-rs --all-targets --offline -- -D warnings
```

Expected: pass.

**Step 6: Commit**

Keep all three technical admission flags false until Task 5:

```bash
git add crates/magic-cfets-rs
git commit -m "feat(cfets): implement official rate and fixing histories"
```

## Task 5: Run bounded live admission and set only proven flags

**Files:**

- Modify: `crates/magic-pbc-rs/src/lib.rs`
- Modify: `crates/magic-cfets-rs/src/lib.rs`
- Create: `crates/magic-pbc-rs/README.md`
- Create: `crates/magic-cfets-rs/README.md`
- Create: `crates/magic-nbs-rs/README.md`

**Step 1: Run two consecutive PBC probes and the load probe**

```bash
cargo run -p magic-pbc-rs --example live_probe --offline
cargo run -p magic-pbc-rs --example live_probe --offline
cargo run -p magic-pbc-rs --example load_probe --offline
```

Expected admission evidence: both live runs return a non-empty strict
money-supply batch with exact table title/unit and the load probe proves
concurrency 1 and at least 1-second start spacing.

If all three pass, change only:

```rust
pub const MONEY_SUPPLY_ADMITTED: bool = true;
```

If any fail, leave it false and record the exact typed failure in the README.
`SOCIAL_FINANCING_ADMITTED` and `REGIONAL_SERIES_ADMITTED` remain false in
either outcome.

**Step 2: Run CFETS probes**

```bash
cargo run -p magic-cfets-rs --example live_probe --offline -- \
  2026-07-28 2026-07-29
cargo run -p magic-cfets-rs --example live_probe --offline -- \
  2026-07-28 2026-07-29
cargo run -p magic-cfets-rs --example load_probe --offline -- \
  2026-07-28 2026-07-29
```

Run these with network access; `--offline` applies only to Cargo dependency
resolution. Admit each of Shibor, LPR, and official FX independently only if
its two live fetches and load probe pass. `DR007_ADMITTED` stays false.

**Step 3: Document source-rights boundary**

Each README lists exact host/path, request bounds, fields retained, source
time semantics, current admission date/result, and unsupported families.
CFETS README must state that public technical reachability is not a grant of
redistribution rights, that the client is bounded on-demand retrieval rather
than a mirror, and that operators must comply with CFETS authorization and
usage terms.

**Step 4: Pass package gates and commit**

```bash
cargo fmt --all -- --check
cargo test -p magic-nbs-rs -p magic-pbc-rs -p magic-cfets-rs \
  --all-targets --offline
cargo clippy -p magic-nbs-rs -p magic-pbc-rs -p magic-cfets-rs \
  --all-targets --offline -- -D warnings
git diff --check
git add crates/magic-nbs-rs crates/magic-pbc-rs crates/magic-cfets-rs
git commit -m "docs: record China official data admission"
```
