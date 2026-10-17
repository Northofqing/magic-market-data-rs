use super::*;
use magic_market_core::{Board, DataStatus, Exchange, Price, PriceLimitRule};

const OBSERVED_METADATA: &str = "1792203685";
const OBSERVED_STATISTICS: &str = "1792203713.191533300";
const SOURCE_AT_STATISTICS: &str = "2026-10-16T16:14:47+08:00";
const METADATA_BATCH: &str = "tdx:1792203685:security-metadata";
const STATISTICS_BATCH: &str = "tencent-web:1792203713.191533300:market-statistics";

fn instrument(code: &str) -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, code, AssetClass::Equity).unwrap()
}

fn metadata_record(instrument: InstrumentId, listed_on: Option<&str>) -> SecurityMetadata {
    SecurityMetadata::new(
        instrument,
        Some("贵州茅台".to_owned()),
        Some(Board::Main),
        Some(false),
        listed_on.map(|value| value.to_owned()),
        PriceLimitRule::new(None, None).unwrap(),
        DataStatus::Unavailable,
        None,
        OBSERVED_METADATA,
        ProviderId::Tdx,
        METADATA_BATCH,
    )
    .unwrap()
}

fn statistics_record(instrument: InstrumentId) -> MarketStatistics {
    let evidence = SourceEvidence::new(ProviderId::Tencent, OBSERVED_STATISTICS, STATISTICS_BATCH)
        .unwrap()
        .with_source_at(SOURCE_AT_STATISTICS)
        .unwrap();
    MarketStatistics::new(
        instrument,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(Price::new(1444.27).unwrap()),
        Some(Price::new(1181.67).unwrap()),
        None,
        evidence,
    )
    .unwrap()
}

fn metadata_batch(record: SecurityMetadata) -> DataBatch<SecurityMetadata> {
    let provenance = Provenance::new("security-metadata", OBSERVED_METADATA)
        .unwrap()
        .with_batch_id(METADATA_BATCH)
        .unwrap();
    DataBatch::strict(vec![record], provenance)
}

fn statistics_batch(record: MarketStatistics) -> DataBatch<MarketStatistics> {
    let provenance = Provenance::new("market-statistics", OBSERVED_STATISTICS)
        .unwrap()
        .with_batch_id(STATISTICS_BATCH)
        .unwrap()
        .with_source_at(SOURCE_AT_STATISTICS)
        .unwrap();
    DataBatch::strict(vec![record], provenance)
}

fn request(
    instruments: Vec<InstrumentId>,
    dates: &[&str],
    include: Option<Vec<QualifiedTradingFactSubset>>,
) -> QualifiedTradingFactsRequest {
    QualifiedTradingFactsRequest {
        instruments,
        dates: dates
            .iter()
            .map(|date| IsoDate::new(*date).unwrap())
            .collect(),
        include,
    }
}

fn row_values(result: QueryResult) -> Vec<serde_json::Value> {
    result
        .records
        .iter()
        .map(|record| serde_json::from_slice(record.data()).unwrap())
        .collect()
}

#[test]
fn plan_defaults_to_lifecycle_and_price_band_and_sorts_dates() {
    let plan = qualified_trading_facts_plan(&request(
        vec![instrument("600519")],
        &["2026-10-16", "2026-10-15"],
        None,
    ))
    .unwrap();
    assert!(plan.lifecycle);
    assert!(plan.price_band);
    assert_eq!(plan.dates[0].as_str(), "2026-10-15");
    assert_eq!(plan.dates[1].as_str(), "2026-10-16");
}

#[test]
fn plan_rejects_duplicate_instruments() {
    let error = qualified_trading_facts_plan(&request(
        vec![instrument("600519"), instrument("600519")],
        &["2026-10-16"],
        None,
    ))
    .unwrap_err();
    assert!(
        matches!(&error, ServiceError::InvalidRequest(message) if message.contains("duplicate instrument")),
        "unexpected error: {error}"
    );
}

#[test]
fn plan_rejects_non_equity_instruments() {
    let index = InstrumentId::new(Exchange::Shanghai, "000300", AssetClass::Index).unwrap();
    let error =
        qualified_trading_facts_plan(&request(vec![index], &["2026-10-16"], None)).unwrap_err();
    assert!(
        matches!(&error, ServiceError::InvalidRequest(message) if message.contains("equity")),
        "unexpected error: {error}"
    );
}

#[test]
fn plan_rejects_dates_outside_the_checked_in_calendar() {
    let error =
        qualified_trading_facts_plan(&request(vec![instrument("600519")], &["2026-10-17"], None))
            .unwrap_err();
    assert!(
        matches!(&error, ServiceError::InvalidRequest(message) if message.contains("trading days")),
        "unexpected error: {error}"
    );
}

#[test]
fn plan_rejects_explicit_suspension_subset() {
    let error = qualified_trading_facts_plan(&request(
        vec![instrument("600519")],
        &["2026-10-16"],
        Some(vec![QualifiedTradingFactSubset::Suspension]),
    ))
    .unwrap_err();
    assert!(
        matches!(&error, ServiceError::Unsupported { reason, .. } if reason.contains("suspension")),
        "unexpected error: {error}"
    );
}

#[test]
fn plan_rejects_empty_include() {
    let error = qualified_trading_facts_plan(&request(
        vec![instrument("600519")],
        &["2026-10-16"],
        Some(Vec::new()),
    ))
    .unwrap_err();
    assert!(
        matches!(&error, ServiceError::InvalidRequest(message) if message.contains("include")),
        "unexpected error: {error}"
    );
}

#[test]
fn plan_rejects_more_than_fifty_instruments() {
    let instruments = (0..51)
        .map(|index| instrument(&format!("60{index:04}")))
        .collect();
    let error =
        qualified_trading_facts_plan(&request(instruments, &["2026-10-16"], None)).unwrap_err();
    assert!(
        matches!(&error, ServiceError::InvalidRequest(message) if message.contains("50")),
        "unexpected error: {error}"
    );
}

#[test]
fn plan_rejects_more_than_five_hundred_rows() {
    let instruments = (0..50)
        .map(|index| instrument(&format!("60{index:04}")))
        .collect();
    let dates = [
        "2026-01-05",
        "2026-01-06",
        "2026-01-07",
        "2026-01-08",
        "2026-01-09",
        "2026-01-12",
        "2026-01-13",
        "2026-01-14",
        "2026-01-15",
        "2026-01-16",
        "2026-01-19",
    ];
    let error = qualified_trading_facts_plan(&request(instruments, &dates, None)).unwrap_err();
    assert!(
        matches!(&error, ServiceError::InvalidRequest(message) if message.contains("rows")),
        "unexpected error: {error}"
    );
}

#[test]
fn rows_carry_listing_state_and_observation_day_price_band() {
    let target = instrument("600519");
    let metadata = metadata_batch(metadata_record(target.clone(), Some("2001-08-27")));
    let statistics = statistics_batch(statistics_record(target.clone()));
    let plan =
        qualified_trading_facts_plan(&request(vec![target], &["2026-10-15", "2026-10-16"], None))
            .unwrap();
    let result =
        qualified_trading_facts_query_result(Some(metadata), Some(statistics), &plan, 1_048_576)
            .unwrap();
    assert!(result.complete);
    assert_eq!(result.provider, "Composition");
    assert_eq!(result.observed_at, "2026-10-17T10:21:25+08:00");
    assert_eq!(
        result.batch_id,
        "tdx:1792203685:security-metadata:qualified-trading-facts"
    );
    let rows = row_values(result);
    assert_eq!(rows.len(), 2);

    let first = &rows[0];
    assert_eq!(first["date"], "2026-10-15");
    assert_eq!(first["quality"], "provider_observed");
    assert_eq!(first["source"], "tdx-security-metadata");
    assert_eq!(first["listed_on"], "2001-08-27");
    assert_eq!(first["listing_state"], "listed");
    assert_eq!(first["evidence"]["provider"], "Tdx");
    assert_eq!(first["evidence"]["batch_id"], METADATA_BATCH);
    assert!(first["source_at"].is_null());
    assert!(first["limit_up"].is_null());
    assert!(first["price_band_source"].is_null());
    assert!(first["price_band_evidence"].is_null());

    let second = &rows[1];
    assert_eq!(second["date"], "2026-10-16");
    assert_eq!(second["limit_up"], 1444.27);
    assert_eq!(second["limit_down"], 1181.67);
    assert_eq!(second["price_band_source"], "tencent-market-statistics");
    assert_eq!(second["price_band_evidence"]["provider"], "Tencent");
    assert_eq!(
        second["price_band_evidence"]["source_at"],
        SOURCE_AT_STATISTICS
    );
    assert_eq!(second["price_band_evidence"]["batch_id"], STATISTICS_BATCH);
}

#[test]
fn listing_state_distinguishes_not_yet_listed_and_unknown() {
    let target = instrument("301999");
    let metadata = metadata_batch(metadata_record(target.clone(), Some("2026-10-16")));
    let plan = qualified_trading_facts_plan(&request(
        vec![target.clone()],
        &["2026-10-15", "2026-10-16"],
        Some(vec![QualifiedTradingFactSubset::Lifecycle]),
    ))
    .unwrap();
    let result =
        qualified_trading_facts_query_result(Some(metadata), None, &plan, 1_048_576).unwrap();
    let rows = row_values(result);
    assert_eq!(rows[0]["listing_state"], "not_yet_listed");
    assert_eq!(rows[1]["listing_state"], "listed");
    assert!(rows[0]["limit_up"].is_null());

    let unknown = metadata_batch(metadata_record(target.clone(), None));
    let plan = qualified_trading_facts_plan(&request(
        vec![target],
        &["2026-10-16"],
        Some(vec![QualifiedTradingFactSubset::Lifecycle]),
    ))
    .unwrap();
    let result =
        qualified_trading_facts_query_result(Some(unknown), None, &plan, 1_048_576).unwrap();
    let rows = row_values(result);
    assert_eq!(rows[0]["listing_state"], "unknown");
    assert!(rows[0]["listed_on"].is_null());
}

#[test]
fn price_band_only_request_omits_lifecycle_fields() {
    let target = instrument("600519");
    let statistics = statistics_batch(statistics_record(target.clone()));
    let plan = qualified_trading_facts_plan(&request(
        vec![target],
        &["2026-10-16"],
        Some(vec![QualifiedTradingFactSubset::PriceBand]),
    ))
    .unwrap();
    let result =
        qualified_trading_facts_query_result(None, Some(statistics), &plan, 1_048_576).unwrap();
    assert_eq!(result.source_at.as_deref(), Some(SOURCE_AT_STATISTICS));
    let rows = row_values(result);
    assert_eq!(rows[0]["source"], "tencent-market-statistics");
    assert!(rows[0]["listed_on"].is_null());
    assert!(rows[0]["listing_state"].is_null());
    assert_eq!(rows[0]["price_band_source"], "tencent-market-statistics");
    assert_eq!(rows[0]["limit_up"], 1444.27);
}

#[test]
fn missing_instrument_in_metadata_batch_fails_closed() {
    let target = instrument("600519");
    let other = instrument("600000");
    let metadata = metadata_batch(metadata_record(other, Some("2001-08-27")));
    let plan = qualified_trading_facts_plan(&request(
        vec![target],
        &["2026-10-16"],
        Some(vec![QualifiedTradingFactSubset::Lifecycle]),
    ))
    .unwrap();
    let error =
        qualified_trading_facts_query_result(Some(metadata), None, &plan, 1_048_576).unwrap_err();
    assert!(
        matches!(&error, ServiceError::FailedPrecondition(message) if message.contains("omitted requested instrument")),
        "unexpected error: {error}"
    );
}

#[test]
fn observed_at_converts_epoch_seconds_to_china_rfc3339() {
    assert_eq!(
        qualified_trading_fact_observed_at("1792203685").unwrap(),
        "2026-10-17T10:21:25+08:00"
    );
    assert_eq!(
        qualified_trading_fact_observed_at("1792203713.191533300").unwrap(),
        "2026-10-17T10:21:53.191+08:00"
    );
    assert!(qualified_trading_fact_observed_at("not-a-timestamp").is_err());
}
