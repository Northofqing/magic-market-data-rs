use magic_market_core::{
    Adjustment, AssetClass, AuctionSnapshot, Bar, BarInterval, BarsRequest, Board, BookLevel,
    DataBatch, DataStatus, Exchange, InstrumentId, Money, MoneyFlow, OrderBook, Price,
    PriceLimitRule, Provenance, ProviderId, Quantity, Quote, Ratio, SecurityMetadata, Trade,
    TradeSide,
};
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

fn quote() -> Quote {
    Quote::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        Price::new(1_308.0).unwrap(),
        Quantity::new(100.0).unwrap(),
        None,
        "2026-07-22T10:00:00+08:00",
        ProviderId::Tdx,
        "batch-1",
    )
    .unwrap()
}

#[test]
fn normalized_batch_round_trips_without_losing_evidence() {
    let provenance = Provenance::new("tdx", "2026-07-22T10:00:01+08:00")
        .unwrap()
        .with_source_at("2026-07-22T10:00:00+08:00")
        .unwrap()
        .with_batch_id("batch-1")
        .unwrap();
    let batch = DataBatch::strict(vec![quote()], provenance);

    let json = serde_json::to_string(&batch).unwrap();
    let decoded: DataBatch<Quote> = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded.records(), batch.records());
    assert_eq!(decoded.provenance(), batch.provenance());
    assert!(decoded.quality().is_complete());
}

fn assert_round_trip<T>(value: T)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(serde_json::from_str::<T>(&json).unwrap(), value);
}

#[test]
fn every_normalized_record_family_round_trips_through_checked_serde() {
    let instrument = InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap();
    let level = BookLevel::unavailable();
    assert_round_trip(quote());
    assert_round_trip(
        MoneyFlow::new(
            instrument.clone(),
            None,
            None,
            None,
            None,
            None,
            DataStatus::Unavailable,
            None,
            "observed",
            ProviderId::Eastmoney,
            "batch-1",
        )
        .unwrap(),
    );
    assert_round_trip(
        OrderBook::new(
            instrument.clone(),
            [level; 5],
            [level; 5],
            None,
            None,
            DataStatus::Unavailable,
            None,
            "observed",
            ProviderId::Tdx,
            "batch-1",
        )
        .unwrap(),
    );
    assert_round_trip(
        AuctionSnapshot::new(
            instrument.clone(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            DataStatus::Unavailable,
            None,
            "observed",
            ProviderId::Eastmoney,
            "batch-1",
        )
        .unwrap(),
    );
    assert_round_trip(
        Trade::new(
            instrument.clone(),
            "10:00:00",
            Price::new(1_308.0).unwrap(),
            Quantity::new(100.0).unwrap(),
            Some(1),
            TradeSide::Unknown(5),
            DataStatus::Unavailable,
            Some("10:00:00".into()),
            "observed",
            ProviderId::Tdx,
            "batch-1",
        )
        .unwrap(),
    );
    assert_round_trip(
        SecurityMetadata::new(
            instrument.clone(),
            Some("贵州茅台".into()),
            Some(Board::Main),
            Some(false),
            None,
            PriceLimitRule::new(None, None).unwrap(),
            DataStatus::Unavailable,
            None,
            "observed",
            ProviderId::Tdx,
            "batch-1",
        )
        .unwrap(),
    );
    assert_round_trip(
        Bar::new(
            instrument,
            BarInterval::Day,
            "2026-07-22",
            "2026-07-22",
            Price::new(100.0).unwrap(),
            Price::new(101.0).unwrap(),
            Price::new(99.0).unwrap(),
            Price::new(100.0).unwrap(),
            Quantity::new(100.0).unwrap(),
            None,
            Adjustment::Unadjusted,
            ProviderId::Tdx,
            "batch-1",
        )
        .unwrap(),
    );
}

#[test]
fn deserialization_cannot_bypass_checked_value_types() {
    assert!(serde_json::from_str::<Price>("0.0").is_err());
    assert!(serde_json::from_str::<Quantity>("-1.0").is_err());
}

#[test]
fn deserialization_cannot_create_an_empty_instrument_code() {
    let json = r#"{"exchange":"Shanghai","code":"   ","asset_class":"Equity"}"#;
    assert!(serde_json::from_str::<InstrumentId>(json).is_err());
}

#[test]
fn deserialization_rejects_contradictory_quality_state() {
    let json = r#"{
        "records": [],
        "provenance": {
            "source": "fixture",
            "source_at": null,
            "fetched_at": "2026-07-22T10:00:00+08:00",
            "batch_id": null
        },
        "quality": {"complete": true, "issues": ["missing page"]}
    }"#;

    assert!(serde_json::from_str::<DataBatch<Quote>>(json).is_err());
}

#[test]
fn deserialization_rejects_empty_evidence_strings() {
    let mut json = serde_json::to_value(quote()).unwrap();
    json["observed_at"] = serde_json::json!(" ");
    assert!(serde_json::from_value::<Quote>(json).is_err());
    assert!(Quote::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        Price::new(1_308.0).unwrap(),
        Quantity::new(100.0).unwrap(),
        None,
        " ",
        ProviderId::Tdx,
        "batch-1",
    )
    .is_err());
}

#[test]
fn quote_deserialization_rejects_available_with_missing_fields() {
    let mut json = serde_json::to_value(quote()).unwrap();
    json["status"] = serde_json::json!("Available");

    assert!(serde_json::from_value::<Quote>(json).is_err());
}

#[test]
fn order_book_deserialization_rejects_half_present_levels() {
    let json = serde_json::json!({"price": null, "quantity": 0.0});
    assert!(serde_json::from_value::<BookLevel>(json).is_err());
}

#[test]
fn records_reject_negative_turnover_but_money_flow_allows_negative_net_values() {
    let instrument = InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap();
    let negative = Money::new(-1.0).unwrap();

    assert!(Quote::new(
        instrument.clone(),
        Price::new(100.0).unwrap(),
        Quantity::new(10.0).unwrap(),
        Some(negative),
        "observed",
        ProviderId::Tdx,
        "batch-1",
    )
    .is_err());
    assert!(AuctionSnapshot::new(
        instrument.clone(),
        None,
        None,
        None,
        None,
        None,
        Some(negative),
        None,
        None,
        None,
        DataStatus::Unavailable,
        None,
        "observed",
        ProviderId::Eastmoney,
        "batch-1",
    )
    .is_err());
    assert!(Bar::new(
        instrument.clone(),
        BarInterval::Day,
        "2026-07-22",
        "2026-07-22",
        Price::new(100.0).unwrap(),
        Price::new(101.0).unwrap(),
        Price::new(99.0).unwrap(),
        Price::new(100.0).unwrap(),
        Quantity::new(10.0).unwrap(),
        Some(negative),
        Adjustment::Unadjusted,
        ProviderId::Tdx,
        "batch-1",
    )
    .is_err());
    assert!(MoneyFlow::new(
        instrument,
        Some(negative),
        None,
        None,
        None,
        None,
        DataStatus::Unavailable,
        None,
        "observed",
        ProviderId::Eastmoney,
        "batch-1",
    )
    .is_ok());
}

#[test]
fn provenance_deserialization_does_not_fabricate_a_missing_batch_id() {
    let json = r#"{
        "source": "fixture",
        "source_at": null,
        "fetched_at": "2026-07-22T10:00:00+08:00",
        "batch_id": null
    }"#;
    let provenance: Provenance = serde_json::from_str(json).unwrap();

    assert_eq!(provenance.batch_id(), None);
    assert_eq!(
        serde_json::to_value(provenance).unwrap()["batch_id"],
        serde_json::Value::Null
    );
}

#[test]
fn bar_deserialization_rechecks_ohlc_invariants() {
    let bar = Bar::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        BarInterval::Day,
        "2026-07-21",
        "2026-07-21",
        Price::new(100.0).unwrap(),
        Price::new(105.0).unwrap(),
        Price::new(95.0).unwrap(),
        Price::new(101.0).unwrap(),
        Quantity::new(100.0).unwrap(),
        None,
        Adjustment::Unadjusted,
        ProviderId::Tdx,
        "batch-1",
    )
    .unwrap();
    let mut json = serde_json::to_value(bar).unwrap();
    json["high"] = serde_json::json!(99.0);

    assert!(serde_json::from_value::<Bar>(json).is_err());
}

#[test]
fn price_limit_deserialization_rechecks_business_rules() {
    let json = serde_json::json!({
        "percent": {"value": -10.0, "unit": "Percent"},
        "version": "rule-v1"
    });
    assert!(serde_json::from_value::<PriceLimitRule>(json).is_err());
    assert!(PriceLimitRule::new(Some(Ratio::decimal(0.1).unwrap()), None).is_err());
}

#[test]
fn request_deserialization_reuses_constructor_validation() {
    let request = BarsRequest::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        BarInterval::Day,
        5,
    )
    .unwrap();
    let mut json = serde_json::to_value(request).unwrap();
    json["limit"] = serde_json::json!(0);
    assert!(serde_json::from_value::<BarsRequest>(json).is_err());

    let json = serde_json::json!({
        "instrument": {"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
        "interval": "Day",
        "start": "2026-07-21",
        "end": null,
        "limit": 5
    });
    assert!(serde_json::from_value::<BarsRequest>(json).is_err());
}

#[test]
fn security_metadata_deserialization_rejects_invalid_listing_date() {
    let metadata = SecurityMetadata::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        Some("贵州茅台".into()),
        Some(Board::Main),
        Some(false),
        None,
        PriceLimitRule::new(None, None).unwrap(),
        DataStatus::Unavailable,
        None,
        "2026-07-22T10:00:00+08:00",
        ProviderId::Tdx,
        "batch-1",
    )
    .unwrap();
    let mut json = serde_json::to_value(metadata).unwrap();
    json["listed_on"] = serde_json::json!("2026-02-30");

    assert!(serde_json::from_value::<SecurityMetadata>(json).is_err());
}
