use magic_eastmoney_rs::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
use magic_market_core::{
    AssetClass, Exchange, InstrumentId, IsoDate, ProviderId, TargetPriceData, TargetPriceRequest,
};
use std::collections::VecDeque;
use std::sync::Mutex;

struct ScriptedTransport {
    bodies: Mutex<VecDeque<Vec<u8>>>,
    requests: Mutex<Vec<String>>,
}

impl ScriptedTransport {
    fn new(bodies: impl IntoIterator<Item = &'static [u8]>) -> Self {
        Self {
            bodies: Mutex::new(bodies.into_iter().map(<[u8]>::to_vec).collect()),
            requests: Mutex::new(Vec::new()),
        }
    }
}

impl EastmoneyTransport for ScriptedTransport {
    fn get(
        &self,
        url: &str,
        _headers: &[(&str, &str)],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.requests.lock().unwrap().push(url.into());
        self.bodies
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| EastmoneyError::Transport("missing scripted response".into()))
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Err(EastmoneyError::Transport("unexpected POST".into()))
    }
}

fn request() -> TargetPriceRequest {
    TargetPriceRequest::new(
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        IsoDate::new("2026-01-01").unwrap(),
        IsoDate::new("2026-07-27").unwrap(),
    )
    .unwrap()
}

#[test]
fn complete_report_page_aggregates_source_ranges_and_unique_contributors() {
    let fixture = r#"{
      "hits":3,"size":3,"TotalPage":1,
      "data":[
        {"infoCode":"R1","stockCode":"600519","stockName":"贵州茅台","market":"SHANGHAI",
         "orgCode":"O1","orgSName":"机构一","publishDate":"2026-04-28 08:00:00",
         "indvAimPriceT":1525,"indvAimPriceL":1525},
        {"infoCode":"R2","stockCode":"600519","stockName":"贵州茅台","market":"SHANGHAI",
         "orgCode":"O2","orgSName":"机构二","publishDate":"2026-07-20 09:00:00",
         "indvAimPriceT":1430,"indvAimPriceL":1430},
        {"infoCode":"R3","stockCode":"600519","stockName":"贵州茅台","market":"SHANGHAI",
         "orgCode":"O1","orgSName":"机构一","publishDate":"2026-07-21",
         "indvAimPriceT":1500,"indvAimPriceL":1400}
      ]
    }"#;
    let client = EastmoneyClient::with_transport(ScriptedTransport::new([fixture.as_bytes()]));
    let batch = client.target_price_consensus(&request()).unwrap();
    let value = &batch.records()[0];
    assert_eq!(value.instrument().code(), "600519");
    assert_eq!(value.instrument_name().as_str(), "贵州茅台");
    assert_eq!(value.sample_count().get(), 3);
    assert_eq!(value.contributor_count().get(), 2);
    assert_eq!(value.low().get(), 1400.0);
    assert_eq!(value.high().get(), 1525.0);
    assert_eq!(value.mean().get(), (1525.0 + 1430.0 + 1450.0) / 3.0);
    assert_eq!(value.input_evidence().len(), 3);
    assert_eq!(value.evidence().provider(), ProviderId::Eastmoney);
    assert_eq!(value.observation_start().as_str(), "2026-04-28");
    assert_eq!(value.observation_end().as_str(), "2026-07-21");
}

#[test]
fn partial_source_fields_duplicate_reports_and_wrong_identity_fail_atomically() {
    for fixture in [
        r#"{"hits":1,"size":1,"TotalPage":1,"data":[{
          "infoCode":"R1","stockCode":"600519","stockName":"贵州茅台","market":"SHANGHAI",
          "orgCode":"O1","orgSName":"机构一","publishDate":"2026-07-20",
          "indvAimPriceT":1430,"indvAimPriceL":null
        }]}"#
            .as_bytes(),
        r#"{"hits":2,"size":2,"TotalPage":1,"data":[
          {"infoCode":"R1","stockCode":"600519","stockName":"贵州茅台","market":"SHANGHAI",
           "orgCode":"O1","orgSName":"机构一","publishDate":"2026-07-20",
           "indvAimPriceT":1430,"indvAimPriceL":1430},
          {"infoCode":"R1","stockCode":"600519","stockName":"贵州茅台","market":"SHANGHAI",
           "orgCode":"O1","orgSName":"机构一","publishDate":"2026-07-21",
           "indvAimPriceT":1400,"indvAimPriceL":1400}
        ]}"#
        .as_bytes(),
        r#"{"hits":1,"size":1,"TotalPage":1,"data":[{
          "infoCode":"R1","stockCode":"000001","stockName":"平安银行","market":"SHENZHEN",
          "orgCode":"O1","orgSName":"机构一","publishDate":"2026-07-20",
          "indvAimPriceT":1430,"indvAimPriceL":1430
        }]}"#
            .as_bytes(),
        r#"{"hits":1,"size":1,"TotalPage":1,"data":[{
          "infoCode":"R1","stockCode":"600519","market":"SHANGHAI",
          "orgCode":"O1","orgSName":"机构一","publishDate":"2026-07-20",
          "indvAimPriceT":1430,"indvAimPriceL":1430
        }]}"#
            .as_bytes(),
        r#"{"hits":2,"size":2,"TotalPage":1,"data":[
          {"infoCode":"R1","stockCode":"600519","stockName":"贵州茅台","market":"SHANGHAI",
           "orgCode":"O1","orgSName":"机构一","publishDate":"2026-07-20",
           "indvAimPriceT":1430,"indvAimPriceL":1430},
          {"infoCode":"R2","stockCode":"600519","stockName":"错误名称","market":"SHANGHAI",
           "orgCode":"O2","orgSName":"机构二","publishDate":"2026-07-21",
           "indvAimPriceT":1400,"indvAimPriceL":1400}
        ]}"#
        .as_bytes(),
    ] {
        let client = EastmoneyClient::with_transport(ScriptedTransport::new([fixture]));
        assert!(client.target_price_consensus(&request()).is_err());
    }
}

#[test]
fn exact_zero_shape_is_typed_verified_empty_while_partial_zero_shapes_fail() {
    let client = EastmoneyClient::with_transport(ScriptedTransport::new([
        br#"{"hits":0,"size":0,"TotalPage":0,"data":[]}"#.as_slice(),
    ]));
    let error = client.target_price_consensus(&request()).unwrap_err();
    let EastmoneyError::VerifiedEmpty(empty) = error else {
        panic!("expected typed verified empty");
    };
    assert_eq!(empty.family(), "target_price_consensus");
    assert!(empty.request_identity().contains("600519"));
    assert_eq!(empty.evidence().provider(), ProviderId::Eastmoney);
    assert_eq!(
        magic_market_core::verify_verified_empty(
            &empty,
            &magic_market_core::ProbeAdmissionPolicy::new(ProviderId::Eastmoney)
        )
        .unwrap(),
        magic_market_core::ProbeStatus::VerifiedEmpty
    );

    for fixture in [
        br#"{"hits":1,"size":0,"TotalPage":0,"data":[]}"#.as_slice(),
        br#"{"hits":0,"size":0,"TotalPage":1,"data":[]}"#.as_slice(),
    ] {
        let client = EastmoneyClient::with_transport(ScriptedTransport::new([fixture]));
        assert!(matches!(
            client.target_price_consensus(&request()),
            Err(EastmoneyError::Protocol(_))
        ));
    }

    let first_page = r#"{
      "hits":2,"size":1,"TotalPage":2,
      "data":[
        {"infoCode":"R1","stockCode":"600519","stockName":"贵州茅台","market":"SHANGHAI",
         "orgCode":"O1","orgSName":"机构一","publishDate":"2026-07-20",
         "indvAimPriceT":1430,"indvAimPriceL":1430}
      ]
    }"#;
    let client = EastmoneyClient::with_transport(ScriptedTransport::new([
        first_page.as_bytes(),
        br#"{"hits":0,"size":0,"TotalPage":0,"data":[]}"#.as_slice(),
    ]));
    assert!(matches!(
        client.target_price_consensus(&request()),
        Err(EastmoneyError::Protocol(message))
            if message.contains("changed from non-empty to exact zero")
    ));
}

#[test]
fn capability_and_provider_trait_are_registered() {
    fn assert_provider<T: TargetPriceData<Error = EastmoneyError>>() {}
    assert_provider::<EastmoneyClient>();
    assert!(EastmoneyClient::research_capabilities().target_price_consensus);
}
