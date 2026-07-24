use super::{join_popularity, parse_quotes, parse_rankings, QuoteWire, RankingWire};
use crate::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
use magic_market_core::{
    AssetClass, Exchange, InstrumentId, PopularityData, PositiveU32, RatioUnit,
};
use std::collections::HashMap;
use std::sync::Mutex;

struct FixtureTransport {
    get_body: Vec<u8>,
    post_body: Vec<u8>,
    seen: Mutex<Vec<String>>,
}

impl EastmoneyTransport for FixtureTransport {
    fn get(
        &self,
        url: &str,
        _headers: &[(&str, &str)],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.seen.lock().unwrap().push(format!("GET {url}"));
        Ok(self.get_body.clone())
    }

    fn post_json(
        &self,
        url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.seen.lock().unwrap().push(format!("POST {url}"));
        Ok(self.post_body.clone())
    }
}

#[test]
fn joins_rank_and_quote_with_separate_evidence() {
    let client = EastmoneyClient::with_transport(FixtureTransport {
        post_body: br#"{"code":0,"message":"OK","data":[
          {"sc":"SH600396","rk":1,"rc":2}
        ]}"#
        .to_vec(),
        get_body: r#"{"rc":0,"data":{"diff":[
          {"f12":"600396","f13":1,"f14":"华电辽能","f2":1308,"f3":9.97}
        ]}}"#
            .as_bytes()
            .to_vec(),
        seen: Mutex::new(Vec::new()),
    });
    let batch = client.popularity(PositiveU32::new(5).unwrap()).unwrap();
    let row = &batch.records()[0];
    assert_eq!(row.instrument.code(), "600396");
    assert_eq!(row.rank.get(), 1);
    assert_eq!(row.price.unwrap().get(), 1308.0);
    assert_eq!(row.name.as_ref().unwrap().as_str(), "华电辽能");
    assert_eq!(row.rank_change.unwrap().get(), 2.0);
    assert_eq!(row.return_ratio.unwrap().get(), 9.97);
    assert_eq!(row.return_ratio.unwrap().unit(), RatioUnit::Percent);
    assert!(row.heat.is_none());
    assert!(row.concepts.is_empty());
    assert!(row.tag.is_none());
    assert_ne!(
        row.quote_evidence.as_ref().unwrap().batch_id(),
        row.evidence.batch_id()
    );
}

#[test]
fn unexpected_rank_shape_fails() {
    let client = EastmoneyClient::with_transport(FixtureTransport {
        post_body: br#"{"code":1,"data":[]}"#.to_vec(),
        get_body: Vec::new(),
        seen: Mutex::new(Vec::new()),
    });
    assert!(client.popularity(PositiveU32::new(1).unwrap()).is_err());
}

#[test]
fn duplicate_ranks_instruments_and_quote_codes_are_rejected() {
    assert!(parse_rankings(
        br#"{"code":0,"data":[
          {"sc":"SH600396","rk":1},
          {"sc":"SZ002475","rk":1}
        ]}"#
    )
    .is_err());
    assert!(parse_rankings(
        br#"{"code":0,"data":[
          {"sc":"SH600396","rk":1},
          {"sc":"SH600396","rk":2}
        ]}"#
    )
    .is_err());
    assert!(parse_quotes(
        br#"{"rc":0,"data":{"diff":[
          {"f12":"600396","f13":1,"f2":1},
          {"f12":"600396","f13":1,"f2":2}
        ]}}"#
    )
    .is_err());
}

#[test]
fn ranking_and_quote_source_exchange_are_cross_checked() {
    assert!(parse_rankings(br#"{"code":0,"data":[{"sc":"SH002475","rk":1}]}"#).is_err());
    assert!(parse_quotes(
        br#"{"rc":0,"data":{"diff":[
          {"f12":"002475","f13":1,"f2":1}
        ]}}"#
    )
    .is_err());
}

#[test]
fn distinct_rankings_and_quotes_preserve_cardinality_without_overwriting() {
    let rankings = parse_rankings(
        br#"{"code":0,"data":[
          {"sc":"SH600396","rk":1},
          {"sc":"SZ002475","rk":2}
        ]}"#,
    )
    .unwrap();
    let quotes = parse_quotes(
        br#"{"rc":0,"data":{"diff":[
          {"f12":"600396","f13":1,"f2":1},
          {"f12":"002475","f13":0,"f2":2}
        ]}}"#,
    )
    .unwrap();
    let batch = join_popularity(rankings, &quotes).unwrap();
    assert_eq!(batch.records().len(), 2);
    assert_eq!(batch.records()[0].instrument.code(), "600396");
    assert_eq!(batch.records()[1].instrument.code(), "002475");
}

#[test]
fn source_cannot_return_more_rankings_than_requested() {
    let client = EastmoneyClient::with_transport(FixtureTransport {
        post_body: br#"{"code":0,"data":[
          {"sc":"SH600396","rk":1},
          {"sc":"SZ002475","rk":2}
        ]}"#
        .to_vec(),
        get_body: Vec::new(),
        seen: Mutex::new(Vec::new()),
    });
    assert!(matches!(
        client.popularity(PositiveU32::new(1).unwrap()),
        Err(EastmoneyError::Protocol(message)) if message.contains("limit")
    ));
}

#[test]
fn public_limit_empty_rankings_and_transport_failures_are_explicit() {
    let client = EastmoneyClient::with_transport(FixtureTransport {
        post_body: br#"{"code":0,"data":[]}"#.to_vec(),
        get_body: Vec::new(),
        seen: Mutex::new(Vec::new()),
    });
    assert!(client.popularity(PositiveU32::new(101).unwrap()).is_err());
    assert!(matches!(
        client.popularity(PositiveU32::new(1).unwrap()),
        Err(EastmoneyError::Protocol(message)) if message.contains("no usable records")
    ));
}

#[test]
fn ranking_parser_covers_all_verified_prefixes_and_schema_failures() {
    let rankings = parse_rankings(
        br#"{"code":0,"data":[
          {"sc":"SH600396","rk":1,"rc":"2.5"},
          {"sc":"SZ002475","rk":2},
          {"sc":"BJ430001","rk":3}
        ]}"#,
    )
    .unwrap();
    assert_eq!(rankings.len(), 3);
    assert_eq!(rankings[0].secid, "1.600396");
    assert_eq!(rankings[1].secid, "0.002475");
    assert_eq!(rankings[2].secid, "0.430001");
    assert_eq!(rankings[0].rank_change, Some(2.5));

    for body in [
        &b"not-json"[..],
        &br#"{"code":1,"data":[]}"#[..],
        &br#"{"code":0}"#[..],
        &br#"{"code":0,"data":[{"sc":"HK000001","rk":1}]}"#[..],
        &br#"{"code":0,"data":[{"sc":"SH600396"}]}"#[..],
        &br#"{"code":0,"data":[{"sc":"SH600396","rk":0}]}"#[..],
        &br#"{"code":0,"data":[{"sc":"SH600396","rk":1,"rc":true}]}"#[..],
    ] {
        assert!(parse_rankings(body).is_err());
    }
}

#[test]
fn quote_parser_covers_array_object_null_and_every_required_field() {
    let object = parse_quotes(
        r#"{"rc":0,"data":{"diff":{"0":{
          "f12":"600396","f13":1,"f14":"华电辽能","f2":"13.08","f3":"9.97"
        }}}}"#
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(object.len(), 1);
    assert!(parse_quotes(br#"{"rc":0,"data":{"diff":null}}"#)
        .unwrap()
        .is_empty());
    for body in [
        &b"not-json"[..],
        &br#"{"rc":1,"data":{"diff":[]}}"#[..],
        &br#"{"rc":0,"data":{}}"#[..],
        &br#"{"rc":0,"data":{"diff":"bad"}}"#[..],
        &br#"{"rc":0,"data":{"diff":[{"f13":1}]}}"#[..],
        &br#"{"rc":0,"data":{"diff":[{"f12":"600396"}]}}"#[..],
        &br#"{"rc":0,"data":{"diff":[{"f12":"600396","f13":1.5}]}}"#[..],
        &br#"{"rc":0,"data":{"diff":[{"f12":"600396","f13":1,"f2":true}]}}"#[..],
    ] {
        assert!(parse_quotes(body).is_err());
    }
}

#[test]
fn join_rejects_unrequested_quotes_and_preserves_absent_quote_fields() {
    let instrument = InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap();
    let ranking = RankingWire {
        instrument: instrument.clone(),
        secid: "1.600396".into(),
        rank: PositiveU32::new(1).unwrap(),
        rank_change: None,
    };
    let batch = join_popularity(vec![ranking], &HashMap::new()).unwrap();
    let record = &batch.records()[0];
    assert!(record.price.is_none());
    assert!(record.name.is_none());
    assert!(record.rank_change.is_none());
    assert!(record.return_ratio.is_none());
    assert!(record.quote_evidence.is_none());

    let ranking = RankingWire {
        instrument,
        secid: "1.600396".into(),
        rank: PositiveU32::new(1).unwrap(),
        rank_change: None,
    };
    let quotes = HashMap::from([("1.600703".into(), QuoteWire::default())]);
    assert!(matches!(
        join_popularity(vec![ranking], &quotes),
        Err(EastmoneyError::Protocol(message)) if message.contains("not requested")
    ));

    let bad_ranking = RankingWire {
        instrument: InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap(),
        secid: "1.600396".into(),
        rank: PositiveU32::new(1).unwrap(),
        rank_change: Some(f64::INFINITY),
    };
    assert!(join_popularity(vec![bad_ranking], &HashMap::new()).is_err());
}
