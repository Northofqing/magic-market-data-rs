use super::{
    exchange_for_code, instrument_from_market, query_url, secid, source_exchange_for_code,
    source_instrument, validate_instrument, validate_source_instrument, validate_source_secucode,
    BatchContext, EastmoneyClient, EastmoneyError,
};
use crate::test_support::ScriptedTransport;
use magic_market_core::{AssetClass, Exchange, InstrumentId, ProviderId};
use std::time::Duration;

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

#[test]
fn constructors_debug_capabilities_transport_wrappers_and_probe_are_truthful() {
    assert!(EastmoneyClient::new().is_ok());
    assert!(EastmoneyClient::with_timeout(Duration::ZERO).is_err());
    let production = EastmoneyClient::with_timeout(Duration::from_millis(1)).unwrap();
    assert_eq!(
        production.load_probe_snapshot().unwrap().request_starts(),
        0
    );
    assert!(format!("{production:?}").contains("EastmoneyClient"));

    let transport = ScriptedTransport::from_bodies([&b"get"[..], &b"post"[..]]);
    let client = EastmoneyClient::with_transport(transport);
    assert_eq!(
        client
            .get("https://push2.eastmoney.com/api", &[("X-Test", "1")])
            .unwrap(),
        b"get"
    );
    assert_eq!(
        client
            .post_json(
                "https://datacenter-web.eastmoney.com/api",
                &[("X-Test", "2")],
                b"{}"
            )
            .unwrap(),
        b"post"
    );
    assert!(matches!(
        client.load_probe_snapshot(),
        Err(EastmoneyError::Unsupported(message)) if message.contains("telemetry")
    ));

    let research = EastmoneyClient::research_capabilities();
    assert!(research.reports);
    assert!(!research.consensus);
    assert!(!research.semantic_search);
    assert!(!research.pdf_download);
    let capital = EastmoneyClient::capital_capabilities();
    assert!(capital.board_flow && capital.margin && capital.block_trades);
    assert!(capital.holder_count && capital.lockups && capital.dividends);
    assert!(!capital.fund_flow_series && !capital.post_close_flow);
    let signals = EastmoneyClient::signal_capabilities();
    assert!(signals.dragon_tiger && signals.popularity);
    assert!(!signals.board_memberships && !signals.market_rankings);
    assert!(!signals.strong_stock_reasons && !signals.concept_hits);
    let pools = EastmoneyClient::limit_pool_capabilities();
    assert!(pools.upper && pools.broken && pools.lower && pools.previous_upper);
    assert!(!pools.reasons);
    let content = EastmoneyClient::content_capabilities();
    assert!(!content.instrument_news && !content.global_news);
    assert!(!content.announcements && !content.investor_questions);
}

#[test]
fn batch_context_validates_family_and_all_evidence_paths() {
    for family in ["", "Upper", "bad_family", "含中文"] {
        assert!(BatchContext::new(family, None).is_err(), "{family}");
    }
    let without_source = BatchContext::new("fixture-2", None).unwrap();
    assert!(without_source.evidence().unwrap().source_at().is_none());
    assert!(without_source
        .evidence_at(None)
        .unwrap()
        .source_at()
        .is_none());
    assert!(without_source
        .finish(vec![1_u8])
        .unwrap()
        .provenance()
        .source_at()
        .is_none());

    let with_source = BatchContext::new("fixture-3", Some("2026-07-24")).unwrap();
    assert_eq!(
        with_source
            .evidence_at(Some("2026-07-23"))
            .unwrap()
            .source_at(),
        Some("2026-07-23")
    );
    assert_eq!(
        with_source
            .finish(vec![1_u8])
            .unwrap()
            .provenance()
            .source_at(),
        Some("2026-07-24")
    );
}

#[test]
fn instrument_identity_helpers_cover_every_exchange_and_failure_shape() {
    let shanghai = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
    let shenzhen = InstrumentId::new(Exchange::Shenzhen, "002475", AssetClass::Equity).unwrap();
    let beijing = InstrumentId::new(Exchange::Beijing, "430001", AssetClass::Equity).unwrap();
    assert_eq!(secid(&shenzhen).unwrap(), "0.002475");
    assert!(secid(&beijing).is_err());

    let index = InstrumentId::new(Exchange::Shanghai, "000001", AssetClass::Index).unwrap();
    assert!(matches!(
        validate_instrument(&index),
        Err(EastmoneyError::Unsupported(_))
    ));
    let malformed = InstrumentId::new(Exchange::Shanghai, "60039A", AssetClass::Equity).unwrap();
    assert!(validate_instrument(&malformed).is_err());
    let unknown = InstrumentId::new(Exchange::Shanghai, "500001", AssetClass::Equity).unwrap();
    assert!(validate_instrument(&unknown).is_err());

    assert_eq!(
        instrument_from_market("600396", 1).unwrap().exchange(),
        Exchange::Shanghai
    );
    assert_eq!(
        instrument_from_market("002475", 0).unwrap().exchange(),
        Exchange::Shenzhen
    );
    assert_eq!(
        instrument_from_market("430001", 0).unwrap().exchange(),
        Exchange::Beijing
    );
    assert!(source_instrument("600396", Exchange::Shenzhen).is_err());
    assert!(validate_source_instrument(&shanghai, "600396", None).is_ok());
    assert!(validate_source_instrument(&shanghai, "002475", None).is_err());
    assert!(validate_source_instrument(&shanghai, "600396", Some(Exchange::Shenzhen)).is_err());

    assert!(validate_source_secucode(&shanghai, "600396.SH").is_ok());
    assert!(validate_source_secucode(&shenzhen, "002475.sz").is_ok());
    assert!(validate_source_secucode(&beijing, "430001.BJ").is_ok());
    assert!(validate_source_secucode(&shanghai, "600396").is_err());
    assert!(validate_source_secucode(&shanghai, "600396.HK").is_err());

    for (code, expected) in [
        ("600396", Exchange::Shanghai),
        ("002475", Exchange::Shenzhen),
        ("300001", Exchange::Shenzhen),
        ("430001", Exchange::Beijing),
        ("830001", Exchange::Beijing),
        ("920001", Exchange::Beijing),
    ] {
        assert_eq!(exchange_for_code(code).unwrap(), expected);
        assert_eq!(source_exchange_for_code(code).unwrap(), expected);
    }
    assert!(exchange_for_code("").is_err());
    assert!(exchange_for_code("500001").is_err());
    assert!(source_exchange_for_code("bad").is_err());
}

#[test]
fn percent_encoding_and_error_categories_cover_all_stable_diagnostics() {
    assert_eq!(
        query_url(
            "https://push2.eastmoney.com/x",
            &[("a-z_~.", "A z/+中".into())]
        ),
        "https://push2.eastmoney.com/x?a-z_~.=A%20z%2F%2B%E4%B8%AD"
    );
    let core = magic_market_core::NonEmptyText::new("").unwrap_err();
    let errors = [
        (
            EastmoneyError::InvalidRequest("x".into()),
            "invalid_request",
        ),
        (EastmoneyError::Transport("x".into()), "transport"),
        (
            EastmoneyError::ResponseTooLarge { limit: 1 },
            "response_too_large",
        ),
        (EastmoneyError::Decode("x".into()), "decode"),
        (EastmoneyError::Protocol("x".into()), "protocol"),
        (EastmoneyError::Unsupported("x".into()), "unsupported"),
        (EastmoneyError::Core(core), "core"),
    ];
    for (error, category) in errors {
        assert_eq!(error.category(), category);
        assert!(!error.to_string().is_empty());
    }
}
