use super::*;
use magic_market_core::{Adjustment, AssetClass, BookLevel, Exchange, Money, Price, Quantity};
use serde_json::json;

const CAPTURE_OBSERVED: &str = "2026-08-14T15:01:00+08:00";
const REQUESTED_AT: &str = "2026-08-14T15:00:30.123456+08:00";

#[derive(Clone)]
struct CapturedTdx {
    quotes: DataBatch<Quote>,
    books: DataBatch<OrderBook>,
    daily: DataBatch<Bar>,
    five_minute: DataBatch<Bar>,
    unavailable_family: Option<&'static str>,
}

impl CapturedTdx {
    fn reject_unavailable(&self, family: &str) -> Result<(), TdxError> {
        if self.unavailable_family == Some(family) {
            Err(TdxError::Connection(format!(
                "offline {family} capture unavailable"
            )))
        } else {
            Ok(())
        }
    }
}

fn capture_provenance(batch: &str, source_at: Option<&str>) -> Provenance {
    let value = Provenance::new("tdx-smart", CAPTURE_OBSERVED)
        .unwrap()
        .with_batch_id(batch)
        .unwrap();
    match source_at {
        Some(source) => value.with_source_at(source).unwrap(),
        None => value,
    }
}

fn captured_tdx() -> CapturedTdx {
    let instrument = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
    let quote = Quote::from_parts(
        instrument.clone(),
        None,
        Price::new(10.5).unwrap(),
        Some(Price::new(10.0).unwrap()),
        Some(Price::new(10.0).unwrap()),
        Some(Price::new(11.0).unwrap()),
        Some(Price::new(9.0).unwrap()),
        None,
        Quantity::new(100.0).unwrap(),
        Some(Money::new(100_000.0).unwrap()),
        DataStatus::Unavailable,
        None,
        CAPTURE_OBSERVED,
        ProviderId::Tdx,
        "quote-capture",
    )
    .unwrap();
    let level = BookLevel::new(
        Some(Price::new(10.0).unwrap()),
        Some(Quantity::new(100.0).unwrap()),
    )
    .unwrap();
    let book = OrderBook::new(
        instrument.clone(),
        [level; 5],
        [level; 5],
        Some(Quantity::new(500.0).unwrap()),
        Some(Quantity::new(500.0).unwrap()),
        DataStatus::Unavailable,
        None,
        CAPTURE_OBSERVED,
        ProviderId::Tdx,
        "book-capture",
    )
    .unwrap();
    let bar = |interval, label, source_at, batch| {
        Bar::new(
            instrument.clone(),
            interval,
            label,
            label,
            Price::new(10.0).unwrap(),
            Price::new(11.0).unwrap(),
            Price::new(9.0).unwrap(),
            Price::new(10.5).unwrap(),
            Quantity::new(100.0).unwrap(),
            Some(Money::new(100_000.0).unwrap()),
            Adjustment::Unadjusted,
            ProviderId::Tdx,
            batch,
        )
        .unwrap()
        .with_source_at(source_at)
        .unwrap()
        .with_observed_at(CAPTURE_OBSERVED)
        .unwrap()
    };
    CapturedTdx {
        unavailable_family: None,
        quotes: DataBatch::strict(vec![quote], capture_provenance("quote-capture", None)),
        books: DataBatch::strict(vec![book], capture_provenance("book-capture", None)),
        daily: DataBatch::strict(
            vec![bar(
                BarInterval::Day,
                "2026-08-14",
                "2026-08-14",
                "day-capture",
            )],
            capture_provenance("day-capture", Some("2026-08-14")),
        ),
        five_minute: DataBatch::strict(
            vec![bar(
                BarInterval::Minute5,
                "2026-08-14 14:55:00",
                "2026-08-14 14:55",
                "minute5-capture",
            )],
            capture_provenance("minute5-capture", Some("2026-08-14 14:55")),
        ),
    }
}

impl TdxDerivedDataSource for CapturedTdx {
    fn connect_to_any(&self, _timeout: Option<f64>) -> Result<bool, TdxError> {
        self.reject_unavailable("connection")?;
        Ok(true)
    }
}

impl RealtimeQuotes for CapturedTdx {
    type Quote = Quote;
    type Error = TdxError;
    fn realtime_quotes(&self, _instruments: &[InstrumentId]) -> Result<DataBatch<Quote>, TdxError> {
        self.reject_unavailable("quote")?;
        Ok(self.quotes.clone())
    }
}

impl OrderBooks for CapturedTdx {
    type Error = TdxError;
    fn order_books(&self, _instruments: &[InstrumentId]) -> Result<DataBatch<OrderBook>, TdxError> {
        self.reject_unavailable("order-book")?;
        Ok(self.books.clone())
    }
}

impl HistoricalBars for CapturedTdx {
    type Bar = Bar;
    type Error = TdxError;
    fn historical_bars(&self, request: &BarsRequest) -> Result<DataBatch<Bar>, TdxError> {
        match request.interval() {
            BarInterval::Day => {
                self.reject_unavailable("daily bars")?;
                Ok(self.daily.clone())
            }
            BarInterval::Minute5 => {
                self.reject_unavailable("five-minute bars")?;
                Ok(self.five_minute.clone())
            }
            _ => Err(TdxError::InvalidData("unexpected fixture interval".into())),
        }
    }
}

fn tdx_fixture_registry(source: CapturedTdx, maximum_payload_bytes: usize) -> OperationRegistry {
    let mut registry = OperationRegistry::all_unadmitted("offline source fixture only");
    let t0_source = source.clone();
    registry
        .register_handler(
            Capability {
                operation: Operation::T0Evidence,
                repository_admitted: true,
                runtime_available: true,
                provider: "Tdx".into(),
                exact_scope: "existing TDX-only bundle with offline external-source fixture".into(),
                blocker: None,
                diagnostic_available: false,
            },
            move |command| {
                execute_tdx_t0_evidence_from_source(&t0_source, 1.0, command, maximum_payload_bytes)
            },
        )
        .unwrap();
    registry
        .register_handler(
            Capability {
                operation: Operation::OutcomeDailyBars,
                repository_admitted: true,
                runtime_available: true,
                provider: "Tdx".into(),
                exact_scope: "existing TDX-only daily bars with offline external-source fixture"
                    .into(),
                blocker: None,
                diagnostic_available: false,
            },
            move |command| {
                execute_tdx_outcome_daily_bars_from_source(
                    &source,
                    1.0,
                    command,
                    maximum_payload_bytes,
                )
            },
        )
        .unwrap();
    registry
}

fn t0_command(requested_at: &str) -> QueryCommand {
    let payload = CanonicalPayload::new(
        "magic.market.t0_evidence.request",
        2,
        serde_json::to_vec(&json!({
            "instruments": [{"exchange":"Shanghai","code":"600396","asset_class":"Equity"}],
            "daily_bar_count": 1,
            "five_minute_bar_count": 1,
            "requested_at": requested_at
        }))
        .unwrap(),
        4096,
    )
    .unwrap();
    QueryCommand::new(
        "tdx-offline-query",
        Operation::T0Evidence,
        Some("Tdx".into()),
        payload,
    )
    .unwrap()
}

fn outcome_command(outcome_due_at: &str) -> QueryCommand {
    QueryCommand::new(
        "tdx-offline-outcome",
        Operation::OutcomeDailyBars,
        Some("Tdx".into()),
        CanonicalPayload::new(
            "magic.market.outcome_daily_bars.request",
            1,
            serde_json::to_vec(&json!({
                "instrument": {"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
                "through": "2026-08-14",
                "limit": 1,
                "outcome_due_at": outcome_due_at
            }))
            .unwrap(),
            4096,
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn tdx_public_outcome_query_preserves_daily_capture_and_exact_outcome_window() {
    let source = captured_tdx();
    let registry = tdx_fixture_registry(source.clone(), 32_768);
    let due_at = "2026-08-14T15:35:00.123456+08:00";
    let result = registry.execute(outcome_command(due_at)).unwrap();
    assert_eq!(result.provider, "Tdx");
    assert!(result.complete && result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.observed_at, CAPTURE_OBSERVED);
    assert_eq!(result.source_at.as_deref(), Some("2026-08-14"));
    assert_eq!(result.records.len(), 1);
    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.outcome_daily_bars");
    assert_eq!(payload.schema_version(), 1);
    let record: OutcomeDailyBarsRecord = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(record.instrument(), source.daily.records()[0].instrument());
    assert_eq!(record.requested_through().as_str(), "2026-08-14");
    assert_eq!(record.requested_limit().get(), 1);
    assert_eq!(record.outcome_due_at(), due_at);
    assert_eq!(record.bars(), source.daily.records());
    assert_eq!(record.algorithm_revision().get(), 1);
    assert_eq!(record.input_evidence().len(), 1);
    let evidence = &record.input_evidence()[0];
    assert_eq!(evidence.provider(), ProviderId::Tdx);
    assert_eq!(evidence.batch_id(), "day-capture");
    assert_eq!(evidence.observed_at(), CAPTURE_OBSERVED);
    assert_eq!(evidence.source_at(), Some("2026-08-14"));
    assert_eq!(record.input_digest_sha256().len(), 64);
    assert!(record
        .input_digest_sha256()
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    assert_eq!(
        result.batch_id,
        format!("tdx:outcome-daily-bars:{}", record.input_digest_sha256())
    );
    let repeated = registry.execute(outcome_command(due_at)).unwrap();
    assert_eq!(repeated.records, result.records);
    assert_eq!(repeated.batch_id, result.batch_id);
    let shifted = registry
        .execute(outcome_command("2026-08-14T15:35:00.123457+08:00"))
        .unwrap();
    assert_ne!(shifted.batch_id, result.batch_id);
}

#[test]
fn tdx_public_outcome_query_rejects_missing_or_conflicting_daily_capture() {
    for corruption in [
        "empty daily bars",
        "incomplete daily bars",
        "wrong source",
        "wrong instrument",
        "wrong newest session",
        "missing source time",
        "missing observed time",
        "contradicting batch identity",
    ] {
        let mut source = captured_tdx();
        match corruption {
            "empty daily bars" => {
                source.daily = DataBatch::strict(Vec::new(), source.daily.provenance().clone());
            }
            "incomplete daily bars" => {
                source.daily = DataBatch::best_effort(
                    source.daily.records().to_vec(),
                    source.daily.provenance().clone(),
                    vec!["daily source window truncated".into()],
                )
                .unwrap();
            }
            "wrong source" => {
                source.daily = DataBatch::strict(
                    source.daily.records().to_vec(),
                    Provenance::new("tdx-other", CAPTURE_OBSERVED).unwrap(),
                );
            }
            "contradicting batch identity" => {
                source.daily = DataBatch::strict(
                    source.daily.records().to_vec(),
                    source
                        .daily
                        .provenance()
                        .clone()
                        .with_batch_id("another-day")
                        .unwrap(),
                );
            }
            _ => {
                let mut value = serde_json::to_value(&source.daily.records()[0]).unwrap();
                match corruption {
                    "wrong instrument" => value["instrument"]["code"] = json!("600519"),
                    "wrong newest session" => {
                        value["bar_start"] = json!("2026-08-13");
                        value["bar_end"] = json!("2026-08-13");
                        value["source_at"] = json!("2026-08-13");
                    }
                    "missing source time" => value["source_at"] = json!(null),
                    "missing observed time" => value["observed_at"] = json!(null),
                    _ => unreachable!("fixed public source fixture cases"),
                }
                source.daily = DataBatch::strict(
                    vec![serde_json::from_value(value).unwrap()],
                    source.daily.provenance().clone(),
                );
            }
        }
        let registry = tdx_fixture_registry(source, 32_768);
        match registry.execute(outcome_command("2026-08-14T15:35:00+08:00")) {
            Err(ServiceError::Unavailable { operation, reason })
                if corruption == "incomplete daily bars" =>
            {
                assert_eq!(operation, Operation::OutcomeDailyBars);
                assert_eq!(
                    reason,
                    "TDX daily bars input is incomplete: daily source window truncated"
                );
            }
            Err(ServiceError::FailedPrecondition(reason))
                if corruption != "incomplete daily bars" =>
            {
                assert!(!reason.is_empty(), "{corruption}");
            }
            Err(error) => panic!("{corruption} misclassified: {error}"),
            Ok(_) => panic!("{corruption} published a successful outcome result"),
        }
    }
}

#[test]
fn tdx_public_queries_keep_source_outages_typed_and_never_return_empty_success() {
    for operation in [Operation::T0Evidence, Operation::OutcomeDailyBars] {
        let families: &[&str] = if operation == Operation::T0Evidence {
            &[
                "connection",
                "quote",
                "order-book",
                "daily bars",
                "five-minute bars",
            ]
        } else {
            &["connection", "daily bars"]
        };
        for &family in families {
            let mut source = captured_tdx();
            source.unavailable_family = Some(family);
            let registry = tdx_fixture_registry(source, 32_768);
            let command = if operation == Operation::T0Evidence {
                t0_command(REQUESTED_AT)
            } else {
                outcome_command("2026-08-14T15:35:00+08:00")
            };
            match registry.execute(command) {
                Err(ServiceError::Unavailable {
                    operation: failed_operation,
                    reason,
                }) => {
                    assert_eq!(failed_operation, operation);
                    assert_eq!(
                        reason,
                        format!("Connection error: offline {family} capture unavailable")
                    );
                }
                Err(error) => panic!("{operation:?}/{family} misclassified: {error}"),
                Ok(_) => {
                    panic!("{operation:?}/{family} returned successful data after source outage")
                }
            }
        }
    }
}

#[test]
fn tdx_public_queries_reject_oversized_output_without_partial_success() {
    let registry = tdx_fixture_registry(captured_tdx(), 1);
    for command in [
        t0_command(REQUESTED_AT),
        outcome_command("2026-08-14T15:35:00+08:00"),
    ] {
        match registry.execute(command) {
            Err(ServiceError::ResourceExhausted(reason)) => {
                assert!(reason.starts_with("payload size "));
                assert!(reason.ends_with(" exceeds maximum 1"));
            }
            Err(error) => panic!("oversized derived response misclassified: {error}"),
            Ok(_) => panic!("oversized derived response became partial or empty success"),
        }
    }
}

#[test]
fn tdx_public_queries_validate_contract_before_attempting_source_connection() {
    let mut source = captured_tdx();
    source.unavailable_family = Some("connection");
    let registry = tdx_fixture_registry(source, 32_768);
    for operation in [Operation::T0Evidence, Operation::OutcomeDailyBars] {
        let template = if operation == Operation::T0Evidence {
            t0_command(REQUESTED_AT)
        } else {
            outcome_command("2026-08-14T15:35:00+08:00")
        };
        let body: serde_json::Value = serde_json::from_slice(template.payload().data()).unwrap();
        let mutations = if operation == Operation::T0Evidence {
            vec![
                ("/instruments", json!([])),
                (
                    "/instruments",
                    json!([body["instruments"][0], body["instruments"][0]]),
                ),
                ("/instruments/0/asset_class", json!("Index")),
                ("/instruments/0/code", json!("60039")),
                ("/daily_bar_count", json!(0)),
                ("/daily_bar_count", json!(801)),
                ("/five_minute_bar_count", json!(801)),
                ("/requested_at", json!("2026-08-14 15:00:30")),
                ("/requested_at", json!(null)),
            ]
        } else {
            vec![
                ("/instrument/asset_class", json!("Index")),
                ("/through", json!("2026-02-30")),
                ("/limit", json!(0)),
                ("/limit", json!(801)),
                (
                    "/outcome_due_at",
                    json!("2026-08-14T14:59:59.999999999+08:00"),
                ),
                ("/outcome_due_at", json!("2026-08-14 15:35:00")),
            ]
        };
        for (field, replacement) in mutations {
            let mut invalid = body.clone();
            *invalid.pointer_mut(field).unwrap() = replacement;
            let command = QueryCommand::new(
                "invalid-offline-derived-contract",
                operation,
                Some("Tdx".into()),
                CanonicalPayload::new(
                    template.payload().schema(),
                    template.payload().schema_version(),
                    serde_json::to_vec(&invalid).unwrap(),
                    4096,
                )
                .unwrap(),
            )
            .unwrap();
            match registry.execute(command) {
                Err(ServiceError::InvalidRequest(reason)) => assert!(!reason.is_empty()),
                Err(error) => {
                    panic!("{operation:?}/{field} reached source or misclassified: {error}")
                }
                Ok(_) => panic!("{operation:?}/{field} accepted invalid caller contract"),
            }
        }
    }
}

#[test]
fn tdx_public_t0_query_preserves_all_four_original_captures_and_requested_instant() {
    let source = captured_tdx();
    let registry = tdx_fixture_registry(source.clone(), 32_768);
    let earliest =
        EvidenceTimestamp::parse_instant(&OffsetDateTime::now_utc().unix_timestamp().to_string())
            .unwrap();
    let result = registry.execute(t0_command(REQUESTED_AT)).unwrap();
    let latest =
        EvidenceTimestamp::parse_instant(&OffsetDateTime::now_utc().unix_timestamp().to_string())
            .unwrap();
    let observed = EvidenceTimestamp::parse_instant(&result.observed_at).unwrap();
    assert!(observed.duration_since(earliest).is_some());
    assert!(latest.duration_since(observed).is_some());
    assert!(result.observed_at.ends_with("+08:00"));
    assert_eq!(result.provider, "Tdx");
    assert!(result.complete && result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.source_at, None);
    assert!(result.batch_id.starts_with("tdx:t0-evidence:"));
    assert_eq!(result.records.len(), 1);
    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.t0_evidence");
    assert_eq!(payload.schema_version(), 2);
    let record: T0EvidenceRecord = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(record.requested_at(), REQUESTED_AT);
    assert_eq!(record.quote(), &source.quotes.records()[0]);
    assert_eq!(record.order_book(), &source.books.records()[0]);
    assert_eq!(record.daily_bars(), source.daily.records());
    assert_eq!(record.five_minute_bars(), source.five_minute.records());
    assert_eq!(record.daily_bar_count().get(), 1);
    assert_eq!(record.five_minute_bar_count().get(), 1);
    assert_eq!(record.algorithm_revision().get(), 2);
    assert_eq!(record.input_evidence().len(), 4);
    for (evidence, batch, source_at) in record
        .input_evidence()
        .iter()
        .zip([
            ("quote-capture", None),
            ("book-capture", None),
            ("day-capture", Some("2026-08-14")),
            ("minute5-capture", Some("2026-08-14 14:55")),
        ])
        .map(|(evidence, (batch, source_at))| (evidence, batch, source_at))
    {
        assert_eq!(evidence.provider(), ProviderId::Tdx);
        assert_eq!(evidence.batch_id(), batch);
        assert_eq!(evidence.observed_at(), CAPTURE_OBSERVED);
        assert_eq!(evidence.source_at(), source_at);
    }
    assert_eq!(record.input_digest_sha256().len(), 64);
    assert!(record
        .input_digest_sha256()
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    let same = registry.execute(t0_command(REQUESTED_AT)).unwrap();
    assert_eq!(same.batch_id, result.batch_id);
    assert_eq!(same.records, result.records);
    let changed = registry
        .execute(t0_command("2026-08-14T15:00:30.123457+08:00"))
        .unwrap();
    let changed_record: T0EvidenceRecord =
        serde_json::from_slice(changed.records[0].data()).unwrap();
    assert_eq!(
        changed_record.requested_at(),
        "2026-08-14T15:00:30.123457+08:00"
    );
    assert_ne!(
        changed_record.input_digest_sha256(),
        record.input_digest_sha256()
    );
    assert_ne!(changed.batch_id, result.batch_id);
}

#[test]
fn tdx_public_t0_query_rejects_incomplete_quote_or_book_input() {
    for family in ["quote", "order-book"] {
        let mut source = captured_tdx();
        let issue = format!("{family} source window truncated");
        if family == "quote" {
            source.quotes = DataBatch::best_effort(
                source.quotes.records().to_vec(),
                source.quotes.provenance().clone(),
                vec![issue.clone()],
            )
            .unwrap();
        } else {
            source.books = DataBatch::best_effort(
                source.books.records().to_vec(),
                source.books.provenance().clone(),
                vec![issue.clone()],
            )
            .unwrap();
        }
        let registry = tdx_fixture_registry(source, 32_768);
        match registry.execute(t0_command(REQUESTED_AT)) {
            Err(ServiceError::Unavailable { operation, reason }) => {
                assert_eq!(operation, Operation::T0Evidence);
                assert_eq!(reason, format!("TDX {family} input is incomplete: {issue}"));
            }
            Err(error) => panic!("incomplete {family} input misclassified: {error}"),
            Ok(_) => panic!("incomplete {family} input published a successful T0 bundle"),
        }
    }
}

#[test]
fn tdx_public_t0_query_requires_complete_daily_and_five_minute_windows() {
    for family in ["daily bars", "five-minute bars"] {
        let mut source = captured_tdx();
        let batch = if family == "daily bars" {
            &mut source.daily
        } else {
            &mut source.five_minute
        };
        *batch = DataBatch::best_effort(
            batch.records().to_vec(),
            batch.provenance().clone(),
            vec!["source window truncated".into()],
        )
        .unwrap();
        let registry = tdx_fixture_registry(source, 32_768);
        match registry.execute(t0_command(REQUESTED_AT)) {
            Err(ServiceError::Unavailable { operation, reason }) => {
                assert_eq!(operation, Operation::T0Evidence);
                assert_eq!(
                    reason,
                    format!("TDX {family} input is incomplete: source window truncated")
                );
            }
            Err(error) => panic!("incomplete {family} misclassified: {error}"),
            Ok(_) => panic!("incomplete {family} published a complete T0 bundle"),
        }
    }
}

#[test]
fn tdx_public_t0_query_rejects_conflicting_source_records_atomically() {
    for corruption in [
        "omitted quote",
        "unexpected quote identity",
        "duplicate book",
        "non-TDX quote",
        "wrong batch source",
        "book source mismatch",
        "daily observation absent",
        "five-minute source absent",
        "daily records absent",
    ] {
        let mut source = captured_tdx();
        match corruption {
            "omitted quote" => {
                source.quotes = DataBatch::strict(Vec::new(), source.quotes.provenance().clone());
            }
            "unexpected quote identity" | "non-TDX quote" => {
                let mut value = serde_json::to_value(&source.quotes.records()[0]).unwrap();
                if corruption == "unexpected quote identity" {
                    value["instrument"]["code"] = json!("600519");
                } else {
                    value["provider"] = json!("Tencent");
                }
                source.quotes = DataBatch::strict(
                    vec![serde_json::from_value(value).unwrap()],
                    source.quotes.provenance().clone(),
                );
            }
            "duplicate book" => {
                let book = source.books.records()[0].clone();
                source.books =
                    DataBatch::strict(vec![book.clone(), book], source.books.provenance().clone());
            }
            "wrong batch source" => {
                source.quotes = DataBatch::strict(
                    source.quotes.records().to_vec(),
                    Provenance::new("another-source", CAPTURE_OBSERVED)
                        .unwrap()
                        .with_batch_id("quote-capture")
                        .unwrap(),
                );
            }
            "book source mismatch" => {
                source.books = DataBatch::strict(
                    source.books.records().to_vec(),
                    source
                        .books
                        .provenance()
                        .clone()
                        .with_source_at(CAPTURE_OBSERVED)
                        .unwrap(),
                );
            }
            "daily observation absent" => {
                let mut value = serde_json::to_value(&source.daily.records()[0]).unwrap();
                value["observed_at"] = serde_json::Value::Null;
                source.daily = DataBatch::strict(
                    vec![serde_json::from_value(value).unwrap()],
                    source.daily.provenance().clone(),
                );
            }
            "five-minute source absent" => {
                let mut value = serde_json::to_value(&source.five_minute.records()[0]).unwrap();
                value["source_at"] = serde_json::Value::Null;
                source.five_minute = DataBatch::strict(
                    vec![serde_json::from_value(value).unwrap()],
                    source.five_minute.provenance().clone(),
                );
            }
            "daily records absent" => {
                source.daily = DataBatch::strict(Vec::new(), source.daily.provenance().clone());
            }
            _ => unreachable!("fixed public source fixture cases"),
        }
        let registry = tdx_fixture_registry(source, 32_768);
        match registry.execute(t0_command(REQUESTED_AT)) {
            Err(ServiceError::FailedPrecondition(reason)) => {
                assert!(!reason.is_empty(), "{corruption}");
            }
            Err(error) => panic!("{corruption} misclassified: {error}"),
            Ok(_) => panic!("{corruption} published a successful T0 bundle"),
        }
    }
}
