# Official Macro Foundation Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan
> task-by-task. Follow red-green-refactor and commit after every task.

**Goal:** Add stable Provider identities, checked economic-series,
reference-rate/fixing, and SEC-filing contracts, plus a reusable bounded HTTPS
transport whose pacing lock is never held during waiting or I/O.

**Architecture:** Core owns market semantics and invariant-preserving
deserialization. `magic-market-transport` owns only HTTP policy, bounded body
reads, typed failures, and clone-shared request-start reservations. It contains
no source URL or payload model. Provider crates consume both in later plans.

**Tech Stack:** Rust 2021, `serde`, `thiserror`, `ureq 2.12.1`, `url 2`,
`magic-market-core = 0.2.0`.

---

## Task 1: Register stable Provider identities

**Files:**

- Modify: `crates/magic-market-core/tests/provider_identity.rs`
- Modify: `crates/magic-market-core/src/provider.rs`

**Step 1: Write the failing serialization test**

Append this exact test:

```rust
#[test]
fn official_macro_and_news_provider_identity_names_are_stable() {
    let cases = [
        (ProviderId::Nbs, "\"Nbs\""),
        (ProviderId::Pbc, "\"Pbc\""),
        (ProviderId::Cfets, "\"Cfets\""),
        (ProviderId::Fred, "\"Fred\""),
        (ProviderId::Imf, "\"Imf\""),
        (ProviderId::WorldBank, "\"WorldBank\""),
        (ProviderId::SecEdgar, "\"SecEdgar\""),
        (ProviderId::XinhuaFinance, "\"XinhuaFinance\""),
        (ProviderId::Yicai, "\"Yicai\""),
        (ProviderId::SecuritiesTimes, "\"SecuritiesTimes\""),
    ];
    for (provider, expected) in cases {
        assert_eq!(serde_json::to_string(&provider).unwrap(), expected);
    }
}
```

Run:

```bash
cargo test -p magic-market-core --test provider_identity \
  official_macro_and_news_provider_identity_names_are_stable --offline
```

Expected: compilation fails because the ten variants do not exist.

**Step 2: Add the enum variants**

Add these variants immediately before `LocalAnalysis` so serialized names stay
exact:

```rust
    Nbs,
    Pbc,
    Cfets,
    Fred,
    Imf,
    WorldBank,
    SecEdgar,
    XinhuaFinance,
    Yicai,
    SecuritiesTimes,
```

Run the same test. Expected: pass.

**Step 3: Commit**

```bash
git add crates/magic-market-core/src/provider.rs \
  crates/magic-market-core/tests/provider_identity.rs
git commit -m "feat(core): register official data provider identities"
```

## Task 2: Add checked economic-series contracts

**Files:**

- Create: `crates/magic-market-core/src/macro_data.rs`
- Create: `crates/magic-market-core/tests/macro_data.rs`
- Modify: `crates/magic-market-core/src/lib.rs`

**Step 1: Write request and period red tests**

Create the test with these cases:

```rust
use magic_market_core::{
    EconomicFrequency, EconomicPeriod, EconomicSeriesKey,
    EconomicSeriesRequest, PositiveU32, ProviderId,
};

fn key(code: &str) -> EconomicSeriesKey {
    EconomicSeriesKey::new(ProviderId::Pbc, "money-supply", code).unwrap()
}

#[test]
fn request_rejects_empty_duplicate_cross_provider_and_reversed_ranges() {
    let jan = EconomicPeriod::month(2025, 1).unwrap();
    let feb = EconomicPeriod::month(2025, 2).unwrap();
    let limit = PositiveU32::new(10).unwrap();
    assert!(EconomicSeriesRequest::new(vec![], jan.clone(), feb.clone(), limit).is_err());
    assert!(EconomicSeriesRequest::new(
        vec![key("M2"), key("M2")],
        jan.clone(),
        feb.clone(),
        limit
    ).is_err());
    let foreign = EconomicSeriesKey::new(ProviderId::Fred, "fred", "M2SL").unwrap();
    assert!(EconomicSeriesRequest::new(
        vec![key("M2"), foreign],
        jan.clone(),
        feb.clone(),
        limit
    ).is_err());
    assert!(EconomicSeriesRequest::new(vec![key("M2")], feb, jan, limit).is_err());
}

#[test]
fn periods_validate_frequency_specific_boundaries() {
    assert_eq!(
        EconomicPeriod::day("2024-02-29").unwrap().frequency(),
        EconomicFrequency::Daily
    );
    assert!(EconomicPeriod::day("2023-02-29").is_err());
    assert!(EconomicPeriod::iso_week(2025, 0).is_err());
    assert!(EconomicPeriod::iso_week(2025, 54).is_err());
    assert!(EconomicPeriod::month(2025, 13).is_err());
    assert!(EconomicPeriod::quarter(2025, 5).is_err());
}
```

Run:

```bash
cargo test -p magic-market-core --test macro_data --offline
```

Expected: unresolved imports.

**Step 2: Implement the checked identity and period types**

Use this public shape in `macro_data.rs`; fields remain private:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EconomicFrequency {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    Annual,
    Irregular,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct EconomicSeriesKey {
    provider: ProviderId,
    namespace: NonEmptyText,
    code: NonEmptyText,
}

impl EconomicSeriesKey {
    pub fn new(
        provider: ProviderId,
        namespace: impl Into<String>,
        code: impl Into<String>,
    ) -> Result<Self, CoreError> {
        Ok(Self {
            provider,
            namespace: NonEmptyText::new(namespace)?,
            code: NonEmptyText::new(code)?,
        })
    }
    pub fn provider(&self) -> ProviderId { self.provider }
    pub fn namespace(&self) -> &str { self.namespace.as_str() }
    pub fn code(&self) -> &str { self.code.as_str() }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum EconomicPeriod {
    Daily(IsoDate),
    Weekly { year: PositiveU32, week: PositiveU32 },
    Monthly { year: PositiveU32, month: PositiveU32 },
    Quarterly { year: PositiveU32, quarter: PositiveU32 },
    Annual { year: PositiveU32 },
    Irregular(NonEmptyText),
}
```

Implement `day`, `iso_week`, `month`, `quarter`, `year`, and `irregular`
constructors. Permit years `1900..=9999`, weeks `1..=53`, months `1..=12`, and
quarters `1..=4`. Implement `frequency()` and a private comparison key.
Implement custom `Deserialize` for both checked types; deserialize through the
constructors, never directly into fields.

**Step 3: Implement the checked request**

Use the following constructor contract:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EconomicSeriesRequest {
    series: Vec<EconomicSeriesKey>,
    start: EconomicPeriod,
    end: EconomicPeriod,
    max_rows: PositiveU32,
}

impl EconomicSeriesRequest {
    pub fn new(
        series: Vec<EconomicSeriesKey>,
        start: EconomicPeriod,
        end: EconomicPeriod,
        max_rows: PositiveU32,
    ) -> Result<Self, CoreError> {
        if series.is_empty() || series.len() > 100 {
            return Err(CoreError::InvalidRequest(
                "economic-series request accepts 1 through 100 series".into(),
            ));
        }
        if max_rows.get() > 10_000 {
            return Err(CoreError::InvalidRequest(
                "economic-series max_rows must not exceed 10000".into(),
            ));
        }
        let provider = series[0].provider();
        let mut seen = std::collections::HashSet::with_capacity(series.len());
        if series.iter().any(|key| key.provider() != provider) {
            return Err(CoreError::InvalidRequest(
                "economic-series request cannot mix providers".into(),
            ));
        }
        if series.iter().any(|key| !seen.insert(key.clone())) {
            return Err(CoreError::InvalidRequest(
                "economic-series request contains duplicate series".into(),
            ));
        }
        if start.frequency() != end.frequency() || start > end {
            return Err(CoreError::InvalidRequest(
                "economic-series range must use one frequency with start not after end".into(),
            ));
        }
        Ok(Self { series, start, end, max_rows })
    }
}
```

Implement accessors, total `Ord`/`PartialOrd` for periods of the same variant,
and custom request `Deserialize`. Cross-frequency ordering must not be used by
the constructor before the equality check.

**Step 4: Write observation invariant red tests**

Add:

```rust
use magic_market_core::{
    EconomicObservation, EconomicObservationStatus, FiniteNumber,
    NonEmptyText, SourceEvidence,
};

fn evidence() -> SourceEvidence {
    SourceEvidence::new(ProviderId::Pbc, "2026-07-29T10:00:00Z", "pbc-1").unwrap()
}

#[test]
fn present_requires_value_and_non_present_forbids_value() {
    let period = EconomicPeriod::month(2025, 1).unwrap();
    assert!(EconomicObservation::new(
        key("M2"), "广义货币(M2)", None, None, period.clone(), None,
        "亿元", None, None, EconomicObservationStatus::Present, None, None,
        evidence(),
    ).is_err());
    assert!(EconomicObservation::new(
        key("M2"), "广义货币(M2)", None, None, period,
        Some(FiniteNumber::new(0.0).unwrap()), "亿元", None, None,
        EconomicObservationStatus::Missing, None, None, evidence(),
    ).is_err());
}

#[test]
fn serde_cannot_bypass_missing_value_invariant() {
    let json = r#"{
      "series":{"provider":"Pbc","namespace":"money-supply","code":"M2"},
      "name":"广义货币(M2)","region_code":null,"region_name":null,
      "period":{"Monthly":{"year":2025,"month":1}},"value":0.0,
      "unit":"亿元","scale":null,"seasonal_adjustment":null,
      "status":"Missing","released_at":null,"revision":null,
      "evidence":{"provider":"Pbc","source_at":null,
        "observed_at":"2026-07-29T10:00:00Z","batch_id":"pbc-1"}
    }"#;
    assert!(serde_json::from_str::<EconomicObservation>(json).is_err());
}
```

Run the package test. Expected: unresolved observation types.

**Step 5: Implement observation, revision, trait, and capabilities**

Use these enums and fields:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EconomicObservationStatus {
    Present,
    Missing,
    NotApplicable,
    Confidential,
    SourceDefined(NonEmptyText),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EconomicRevisionKind {
    Preliminary,
    Revised,
    Final,
    SourceDefined(NonEmptyText),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicRevision {
    pub kind: EconomicRevisionKind,
    pub label: Option<NonEmptyText>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EconomicObservation {
    series: EconomicSeriesKey,
    name: NonEmptyText,
    region_code: Option<NonEmptyText>,
    region_name: Option<NonEmptyText>,
    period: EconomicPeriod,
    value: Option<FiniteNumber>,
    unit: NonEmptyText,
    scale: Option<NonEmptyText>,
    seasonal_adjustment: Option<NonEmptyText>,
    status: EconomicObservationStatus,
    released_at: Option<NonEmptyText>,
    revision: Option<EconomicRevision>,
    evidence: SourceEvidence,
}
```

Implement the constructor in the test's argument order. Enforce:

```rust
match (&status, value) {
    (EconomicObservationStatus::Present, Some(_)) => {}
    (EconomicObservationStatus::Present, None) => {
        return Err(CoreError::InvalidRequest(
            "present economic observation requires a value".into(),
        ));
    }
    (_, None) => {}
    (_, Some(_)) => {
        return Err(CoreError::InvalidRequest(
            "non-present economic observation cannot contain a value".into(),
        ));
    }
}
```

Use a scoped `#[allow(clippy::too_many_arguments)]` on checked record
constructors whose source facts exceed Clippy's argument threshold; do not add
a module/crate-wide allowance.

Also require `series.provider() == evidence.provider()` and require
`released_at.as_ref().map(NonEmptyText::as_str) == evidence.source_at()` so
record fields cannot contradict routing evidence. Add accessors and custom
`Deserialize`. Implement all evidence methods:

```rust
impl SourcedRecord for EconomicObservation {
    fn provider_id(&self) -> ProviderId { self.evidence.provider() }
    fn evidence_batch_id(&self) -> &str { self.evidence.batch_id() }
    fn evidence_source_at(&self) -> Option<&str> { self.evidence.source_at() }
    fn evidence_observed_at(&self) -> Option<&str> {
        Some(self.evidence.observed_at())
    }
}

pub trait EconomicSeriesProvider {
    type Error: std::error::Error + Send + Sync + 'static;
    fn economic_series(
        &self,
        request: &EconomicSeriesRequest,
    ) -> Result<DataBatch<EconomicObservation>, Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EconomicDataCapabilities {
    pub economic_series: bool,
    pub regional_series: bool,
}
```

Export every public type from `lib.rs`. Run:

```bash
cargo test -p magic-market-core --test macro_data --offline
cargo test -p magic-market-core --lib --offline
```

Expected: pass.

**Step 6: Commit**

```bash
git add crates/magic-market-core/src/macro_data.rs \
  crates/magic-market-core/src/lib.rs \
  crates/magic-market-core/tests/macro_data.rs
git commit -m "feat(core): add checked economic series contracts"
```

## Task 3: Add benchmark-rate and official-fixing contracts

**Files:**

- Create: `crates/magic-market-core/src/reference_data.rs`
- Create: `crates/magic-market-core/tests/reference_data.rs`
- Modify: `crates/magic-market-core/src/lib.rs`

**Step 1: Write red tests for identities, bounds, and positive fixings**

```rust
use magic_market_core::{
    CurrencyCode, FiniteNumber, IsoDate, OfficialFxFixing,
    OfficialFxFixingIdentity, OfficialFxFixingRequest, PositiveU32,
    ProviderId, ReferenceRateIdentity, ReferenceRateKind,
    ReferenceRateRequest, ReferenceTenor, SourceEvidence,
};

#[test]
fn currency_and_request_identities_are_checked() {
    assert_eq!(CurrencyCode::new("cny").unwrap().as_str(), "CNY");
    assert!(CurrencyCode::new("CN").is_err());
    assert!(CurrencyCode::new("C1Y").is_err());
    let pair = OfficialFxFixingIdentity::new(
        ProviderId::Cfets,
        CurrencyCode::new("USD").unwrap(),
        CurrencyCode::new("CNY").unwrap(),
    ).unwrap();
    assert!(OfficialFxFixingRequest::new(
        vec![pair.clone(), pair],
        IsoDate::new("2026-07-01").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        PositiveU32::new(50).unwrap(),
    ).is_err());
}

#[test]
fn official_fixing_requires_positive_value_and_quotation_base() {
    let evidence = SourceEvidence::new(
        ProviderId::Cfets,
        "2026-07-29T02:00:00Z",
        "cfets-1",
    ).unwrap();
    assert!(OfficialFxFixing::new(
        CurrencyCode::new("USD").unwrap(),
        CurrencyCode::new("CNY").unwrap(),
        IsoDate::new("2026-07-29").unwrap(),
        FiniteNumber::new(0.0).unwrap(),
        PositiveU32::new(1).unwrap(),
        None, None, evidence,
    ).is_err());
}
```

Run:

```bash
cargo test -p magic-market-core --test reference_data --offline
```

Expected: unresolved imports.

**Step 2: Implement exact checked public shapes**

Implement:

```rust
pub enum ReferenceTenor {
    Overnight, OneWeek, TwoWeeks, OneMonth, ThreeMonths, SixMonths,
    NineMonths, OneYear, OverFiveYears,
}

pub enum ReferenceRateKind {
    Shibor(ReferenceTenor),
    LoanPrimeRate(ReferenceTenor),
    Dr007,
    SourceDefined(NonEmptyText),
}

pub struct ReferenceRateIdentity {
    provider: ProviderId,
    kind: ReferenceRateKind,
}

pub struct ReferenceRateRequest {
    rates: Vec<ReferenceRateIdentity>,
    start: IsoDate,
    end: IsoDate,
    max_rows: PositiveU32,
}

pub struct ReferenceRateObservation {
    identity: ReferenceRateIdentity,
    fixing_date: IsoDate,
    rate: FiniteNumber,
    unit: RatioUnit,
    published_at: Option<NonEmptyText>,
    revision: Option<EconomicRevision>,
    evidence: SourceEvidence,
}
```

`ReferenceRateIdentity::new` rejects invalid pairings: LPR accepts only
`OneYear` and `OverFiveYears`; Shibor accepts only the eight published Shibor
tenors. `ReferenceRateRequest::new` accepts 1 through 50 unique same-provider
identities, `start <= end`, and `max_rows <= 10_000`.

Implement:

```rust
pub struct CurrencyCode(String);

impl CurrencyCode {
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into().to_ascii_uppercase();
        if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            return Err(CoreError::InvalidValue {
                field: "currency_code",
                value,
                reason: "must contain exactly three ASCII letters",
            });
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}
```

Add `OfficialFxFixingIdentity`, `OfficialFxFixingRequest`, and
`OfficialFxFixing` with fields from the design. Reject same-currency pairs,
duplicates, cross-provider requests, reversed ranges, zero/negative values,
and `max_rows > 10_000`. Both record constructors require their identity
Provider to equal evidence Provider and require
`published_at.as_ref().map(NonEmptyText::as_str) == evidence.source_at()`.
Implement checked custom deserialization for every type with private fields.

Add:

```rust
pub trait ReferenceRateProvider {
    type Error: std::error::Error + Send + Sync + 'static;
    fn reference_rates(
        &self,
        request: &ReferenceRateRequest,
    ) -> Result<DataBatch<ReferenceRateObservation>, Self::Error>;
}

pub trait OfficialFxFixingProvider {
    type Error: std::error::Error + Send + Sync + 'static;
    fn official_fx_fixings(
        &self,
        request: &OfficialFxFixingRequest,
    ) -> Result<DataBatch<OfficialFxFixing>, Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ReferenceDataCapabilities {
    pub benchmark_rates: bool,
    pub official_fx_fixings: bool,
}
```

Implement all four evidence methods for both record types and export them from
`lib.rs`.

**Step 3: Prove serde invariants and pass tests**

Add JSON bypass tests for a zero fixing, duplicate request identities, and LPR
with `OneMonth`. Run:

```bash
cargo test -p magic-market-core --test reference_data --offline
cargo test -p magic-market-core --lib --offline
```

Expected: pass.

**Step 4: Commit**

```bash
git add crates/magic-market-core/src/reference_data.rs \
  crates/magic-market-core/src/lib.rs \
  crates/magic-market-core/tests/reference_data.rs
git commit -m "feat(core): add official reference data contracts"
```

## Task 4: Add checked SEC filing contracts

**Files:**

- Create: `crates/magic-market-core/src/filings.rs`
- Create: `crates/magic-market-core/tests/filings.rs`
- Modify: `crates/magic-market-core/src/lib.rs`

**Step 1: Write the red tests**

```rust
use magic_market_core::{
    CompanyFilingRequest, NonEmptyText, PositiveU32, SecCompanyIdentity,
};

#[test]
fn cik_is_normalized_and_company_requests_are_bounded() {
    let company = SecCompanyIdentity::new("320193", Some("AAPL")).unwrap();
    assert_eq!(company.cik(), "0000320193");
    assert_eq!(company.ticker(), Some("AAPL"));
    assert!(SecCompanyIdentity::new("12345678901", None::<String>).is_err());
    assert!(CompanyFilingRequest::new(
        vec![company.clone(), company],
        vec![],
        None,
        None,
        PositiveU32::new(10).unwrap(),
    ).is_err());
}

#[test]
fn accession_and_primary_document_are_path_safe() {
    assert!(magic_market_core::SecAccessionNumber::new(
        "0000320193-25-000079"
    ).is_ok());
    assert!(magic_market_core::SecAccessionNumber::new("../000079").is_err());
    assert!(magic_market_core::SecPrimaryDocument::new("../report.htm").is_err());
}
```

Run:

```bash
cargo test -p magic-market-core --test filings --offline
```

Expected: unresolved imports.

**Step 2: Implement identities and request**

Implement `SecCompanyIdentity` with 1 through 10 input digits normalized using:

```rust
let cik = value.into();
if cik.is_empty() || cik.len() > 10 || !cik.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(CoreError::InvalidValue {
        field: "sec_cik",
        value: cik,
        reason: "must contain 1 through 10 ASCII digits",
    });
}
let cik = format!("{cik:0>10}");
```

Normalize optional tickers to uppercase and accept only 1 through 10 ASCII
letters, digits, `-`, or `.`. Implement `SecAccessionNumber` as exactly
`##########-##-######`; expose `without_hyphens()`. Implement
`SecPrimaryDocument` as one safe path segment ending in `.htm`, `.html`,
`.txt`, or `.xml`, rejecting slash, backslash, dot segments, query, fragment,
and control characters.

Implement `CompanyFilingRequest` with:

```rust
pub fn new(
    companies: Vec<SecCompanyIdentity>,
    forms: Vec<NonEmptyText>,
    start: Option<IsoDate>,
    end: Option<IsoDate>,
    max_records: PositiveU32,
) -> Result<Self, CoreError>
```

Accept 1 through 100 unique companies, at most 20 unique form strings, either
both dates or neither, `start <= end`, and `max_records <= 1_000`.

**Step 3: Implement filing records and trait**

Use:

```rust
pub struct CompanyFiling {
    company: SecCompanyIdentity,
    company_name: NonEmptyText,
    form: NonEmptyText,
    filing_date: IsoDate,
    report_period: Option<IsoDate>,
    accession: SecAccessionNumber,
    primary_document: SecPrimaryDocument,
    filing_index_url: HttpsUrl,
    primary_document_url: HttpsUrl,
    accepted_at: Option<NonEmptyText>,
    evidence: SourceEvidence,
}

pub trait CompanyFilingsProvider {
    type Error: std::error::Error + Send + Sync + 'static;
    fn company_filings(
        &self,
        request: &CompanyFilingRequest,
    ) -> Result<DataBatch<CompanyFiling>, Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FilingCapabilities {
    pub filing_metadata: bool,
    pub filing_documents: bool,
    pub xbrl_facts: bool,
}
```

`CompanyFiling::new` checks that both URLs are HTTPS; SEC host/path enforcement
remains in the SEC Provider because Core must not own one source's hostname.
It also requires
`accepted_at.as_ref().map(NonEmptyText::as_str) == evidence.source_at()`.
Implement private fields, accessors, custom deserialization, and all four
evidence methods.

**Step 4: Add serde bypass tests and pass**

Test malformed accession, unsafe primary document, reversed date range,
accepted/evidence timestamp disagreement, and `max_records=1001`. Run:

```bash
cargo test -p magic-market-core --test filings --offline
cargo test -p magic-market-core --lib --offline
```

Expected: pass.

**Step 5: Commit**

```bash
git add crates/magic-market-core/src/filings.rs \
  crates/magic-market-core/src/lib.rs \
  crates/magic-market-core/tests/filings.rs
git commit -m "feat(core): add SEC filing metadata contracts"
```

## Task 5: Add shared bounded HTTPS transport

**Files:**

- Modify: `Cargo.toml`
- Create: `crates/magic-market-transport/Cargo.toml`
- Create: `crates/magic-market-transport/src/lib.rs`
- Create: `crates/magic-market-transport/src/gate.rs`
- Create: `crates/magic-market-transport/src/http.rs`
- Create: `crates/magic-market-transport/tests/request_gate.rs`
- Create: `crates/magic-market-transport/tests/http_policy.rs`
- Modify: `Cargo.lock`

**Step 1: Register the crate and write the gate red test**

Manifest:

```toml
[package]
name = "magic-market-transport"
version = "0.2.0"
edition.workspace = true
license.workspace = true

[dependencies]
thiserror = { workspace = true }
ureq = { version = "=2.12.1", default-features = false, features = ["tls"] }
url = "=2.5.4"

[lints]
workspace = true
```

Add `crates/magic-market-transport` to workspace members. Test:

```rust
use magic_market_transport::RequestGate;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

#[test]
fn reservation_lock_is_not_held_during_wait_or_io() {
    let gate = Arc::new(RequestGate::new(Duration::from_millis(40)).unwrap());
    gate.wait_for_turn().unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let second = {
        let gate = Arc::clone(&gate);
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            barrier.wait();
            let started = Instant::now();
            gate.wait_for_turn().unwrap();
            started.elapsed()
        })
    };
    barrier.wait();
    let reservation_started = Instant::now();
    let _third_reservation = gate.reserve().unwrap();
    assert!(reservation_started.elapsed() < Duration::from_millis(20));
    assert!(second.join().unwrap() >= Duration::from_millis(35));
}
```

Run:

```bash
cargo test -p magic-market-transport --test request_gate --offline
```

Expected: unresolved crate/API.

**Step 2: Implement reservation pacing**

Use:

```rust
#[derive(Debug)]
pub struct RequestGate {
    interval: Duration,
    next_start: Mutex<Instant>,
}

impl RequestGate {
    pub fn new(interval: Duration) -> Result<Self, TransportError> {
        if interval.is_zero() {
            return Err(TransportError::InvalidRequest(
                "request interval must be positive".into(),
            ));
        }
        Ok(Self {
            interval,
            next_start: Mutex::new(Instant::now()),
        })
    }

    pub fn reserve(&self) -> Result<Instant, TransportError> {
        let now = Instant::now();
        let mut next = self.next_start.lock().map_err(|_| {
            TransportError::Internal("request gate lock poisoned".into())
        })?;
        let reserved = (*next).max(now);
        *next = reserved.checked_add(self.interval).ok_or_else(|| {
            TransportError::ResourceLimit("request gate instant overflow".into())
        })?;
        Ok(reserved)
    }

    pub fn wait_for_turn(&self) -> Result<(), TransportError> {
        let reserved = self.reserve()?;
        if let Some(wait) = reserved.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
        Ok(())
    }
}
```

The mutex guard is dropped when `reserve` returns, before `sleep`. No HTTP API
accepts or retains the guard. Run the gate test. Expected: pass.

**Step 3: Write HTTP policy red tests**

Test this exact public surface:

```rust
use magic_market_transport::{
    EndpointPolicy, HttpMethod, HttpRequest, HttpResponse, MediaType,
    TransportError,
};
use std::time::Duration;

fn policy() -> EndpointPolicy {
    EndpointPolicy::new(
        "api.example.test",
        vec!["/v1/data".into()],
        vec!["series_id".into(), "start".into(), "end".into()],
        vec![MediaType::Json],
        1024,
        Duration::from_secs(10),
    ).unwrap()
}

#[test]
fn policy_rejects_redirect_hosts_query_keys_and_oversize_bodies() {
    assert!(HttpRequest::new(
        HttpMethod::Get,
        "https://api.example.test/v1/data?secret=x",
        vec![],
        vec![],
    ).and_then(|request| policy().validate_request(&request)).is_err());
    let response = HttpResponse::new(
        200,
        "https://other.example.test/v1/data",
        Some("application/json".into()),
        vec![b'x'; 10],
    );
    assert!(policy().validate_response(response).is_err());
    let response = HttpResponse::new(
        200,
        "https://api.example.test/v1/data",
        Some("application/json".into()),
        vec![b'x'; 1025],
    );
    assert!(matches!(
        policy().validate_response(response),
        Err(TransportError::ResourceLimit(_))
    ));
}
```

Run:

```bash
cargo test -p magic-market-transport --test http_policy --offline
```

Expected: unresolved HTTP types.

**Step 4: Implement typed HTTP primitives**

Expose:

```rust
pub enum HttpMethod { Get, Post }
pub enum MediaType { Json, Html, Xml, PlainText }
pub struct HttpRequest {
    method: HttpMethod,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}
pub struct HttpResponse {
    status: u16,
    final_url: String,
    content_type: Option<String>,
    body: Vec<u8>,
}
pub trait HttpTransport: Send + Sync {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError>;
}
```

`EndpointPolicy::new` checks exact ASCII hostname, non-empty absolute path
prefixes, unique query keys, timeout `1..=60s`, and body ceiling
`1..=16_777_216`. `validate_request` uses `url::Url`, requires HTTPS, no
username/password, no non-443 explicit port, exact host, an allowed path
prefix at a path-segment boundary, and only allowlisted query keys.
`HttpRequest::new` rejects duplicate/invalid header names, CR/LF/control
characters, and `Cookie`, `Authorization`, or `Proxy-Authorization`.
`validate_response` requires status 200, a final URL exactly equal to the
validated requested URL because redirects are disabled, a closed MIME match
ignoring parameters, identity/no content encoding, and the configured body
ceiling.

Use typed errors:

```rust
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("invalid transport request: {0}")]
    InvalidRequest(String),
    #[error("authentication failed: {0}")]
    Authentication(String),
    #[error("HTTP transport failed: {0}")]
    Network(String),
    #[error("HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("redirect or final URL rejected: {0}")]
    Redirect(String),
    #[error("response media type rejected: {0}")]
    MediaType(String),
    #[error("transport resource limit: {0}")]
    ResourceLimit(String),
    #[error("transport internal failure: {0}")]
    Internal(String),
}
```

`UreqTransport` uses redirects `0`, sends `Accept-Encoding: identity`, uses the
validated timeout and a bounded `Read::take(max_bytes + 1)`, rejects any
non-identity `Content-Encoding`, and returns `HttpStatus { status: 429 }`
without retry. Its error formatting must never include request headers or URL
query values.

**Step 5: Add injected and no-redirect tests**

Cover:

- status 429 remains typed;
- `application/json; charset=utf-8` matches JSON;
- missing MIME fails;
- an explicit redirect status fails;
- a body exactly at the ceiling passes and one byte over fails;
- URLs with credentials, fragments, unknown query keys, or alternate ports
  fail before I/O;
- `Debug` for `HttpRequest` prints method/host/path but redacts header values,
  query values, and body.

Run:

```bash
cargo test -p magic-market-transport --all-targets --offline
cargo clippy -p magic-market-transport --all-targets --offline -- -D warnings
```

Expected: pass.

**Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/magic-market-transport
git commit -m "feat(transport): add bounded shared HTTPS support"
```

## Task 6: Foundation checkpoint

**Files:**

- Verify only

Run:

```bash
cargo fmt --all -- --check
cargo test -p magic-market-core --all-targets --offline
cargo test -p magic-market-transport --all-targets --offline
cargo clippy -p magic-market-core -p magic-market-transport \
  --all-targets --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc \
  -p magic-market-core -p magic-market-transport --no-deps --offline
git diff --check
```

Expected: every command passes and `git status --short` is empty. If formatting
changes are required, run `cargo fmt --all`, rerun the checkpoint, and commit
only the formatting changes as:

```bash
git add crates/magic-market-core crates/magic-market-transport
git commit -m "style: format official data foundation"
```
