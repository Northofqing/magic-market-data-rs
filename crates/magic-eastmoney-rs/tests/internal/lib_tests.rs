use super::{
    instrument_from_market, query_url, secid, validate_instrument, BatchContext, EastmoneyClient,
};
use magic_market_core::{AssetClass, Exchange, InstrumentId, ProviderId};

#[test]
fn query_values_are_utf8_percent_encoded() {
    assert_eq!(
        query_url(
            "https://push2.eastmoney.com/x",
            &[("filter", "电力 A".into())]
        ),
        "https://push2.eastmoney.com/x?filter=%E7%94%B5%E5%8A%9B%20A"
    );
}

#[test]
fn secid_preserves_verified_exchange_routing() {
    let instrument = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
    assert_eq!(secid(&instrument).unwrap(), "1.600396");
}

#[test]
fn code_prefix_must_match_declared_and_source_exchange() {
    let mismatches = [
        (Exchange::Shanghai, "002475"),
        (Exchange::Shenzhen, "600396"),
        (Exchange::Beijing, "300001"),
    ];
    for (exchange, code) in mismatches {
        let instrument = InstrumentId::new(exchange, code, AssetClass::Equity).unwrap();
        assert!(matches!(
            validate_instrument(&instrument),
            Err(super::EastmoneyError::InvalidRequest(message))
                if message.contains("exchange")
        ));
    }
    assert!(matches!(
        instrument_from_market("002475", 1),
        Err(super::EastmoneyError::Protocol(message))
            if message.contains("market")
    ));
}

#[test]
fn unverified_fund_flow_is_not_admitted_as_a_capability() {
    assert!(!EastmoneyClient::capital_capabilities().fund_flow_series);
}

#[test]
fn keyword_only_instrument_news_is_not_admitted_as_a_capability() {
    assert!(!EastmoneyClient::content_capabilities().instrument_news);
}

#[test]
fn batch_and_record_evidence_share_identity() {
    let context = BatchContext::new("fixture", Some("2026-07-23")).unwrap();
    let evidence = context.evidence().unwrap();
    let batch = context.finish(vec![1_u8]).unwrap();
    assert_eq!(evidence.provider(), ProviderId::Eastmoney);
    assert_eq!(Some(evidence.batch_id()), batch.provenance().batch_id());
    assert_eq!(evidence.source_at(), Some("2026-07-23"));
}

#[test]
fn empty_batches_are_explicit_protocol_failures() {
    let context = BatchContext::new("fixture", None).unwrap();
    assert!(context.finish::<u8>(Vec::new()).is_err());
}
