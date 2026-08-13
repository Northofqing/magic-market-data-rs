use magic_market_core::{
    EconomicPeriod, EconomicSeriesKey, EconomicSeriesProvider, EconomicSeriesRequest, PositiveU32,
    ProviderId,
};
use magic_market_transport::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use magic_nbs_rs::NbsClient;
use serde_json::Value;
use std::sync::{Arc, Mutex};

struct FormalFixture {
    call: Mutex<usize>,
}

struct RegionalFixture {
    call: Mutex<usize>,
}

impl HttpTransport for RegionalFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let mut call = self.call.lock().unwrap();
        let response = match *call {
            0 => catalog(
                "69c574ab128a44e595cc0b24502b771b",
                "",
                "分省月度数据",
                3,
                false,
                "f4c6cd795fea436c807163397dd36b98",
            ),
            1 => catalog(
                "f4c6cd795fea436c807163397dd36b98",
                "f4c6cd795fea436c807163397dd36b98",
                "价格指数",
                4,
                false,
                "fce37cb7a3124492ba7d1467d70b40c1",
            ),
            2 => catalog(
                "fce37cb7a3124492ba7d1467d70b40c1",
                "fce37cb7a3124492ba7d1467d70b40c1",
                "居民消费价格分类指数",
                5,
                false,
                "6a27e7bf8c2c4ce4b7d58d56d806f4a5",
            ),
            3 => catalog(
                "6a27e7bf8c2c4ce4b7d58d56d806f4a5",
                "6a27e7bf8c2c4ce4b7d58d56d806f4a5",
                "居民消费价格分类指数 (上年同月=100) (2026-)",
                6,
                true,
                "b5e122302ad745358ba7415bd4a22c2f",
            ),
            4 => {
                serde_json::json!({"data":{"total":1,"list":[{"_id":"02d75ee002764e2ea53f263e52109e8f","i_showname":"居民消费价格指数 (上年同月=100) ","du_name":"%","catalogid":"b5e122302ad745358ba7415bd4a22c2f"}]},"success":true,"state":20000})
            }
            5 => {
                serde_json::json!({"data":[{"_id":"a10dceae75d245008bf4b9a0e6fe1d55","name":"全部地区","treeinfo_pid":"6a99abef22d44119bdf5f8e2451ac390","treeinfo_level":2}],"success":true,"state":20000})
            }
            6 => {
                serde_json::json!({"data":[{"catalog_id":"a10dceae75d245008bf4b9a0e6fe1d55","name_value":"110000000000","show_name":"北京市","name_text":"北京市"}],"success":true,"state":20000})
            }
            7 => {
                let body: Value = serde_json::from_slice(request.body()).unwrap();
                assert_eq!(body["cid"], "b5e122302ad745358ba7415bd4a22c2f");
                assert_eq!(body["indicatorIds"][0], "02d75ee002764e2ea53f263e52109e8f");
                assert_eq!(body["das"][0]["value"], "110000000000");
                assert_eq!(body["das"][0]["text"], "北京市");
                assert_eq!(body["dts"][0], "202607MM");
                serde_json::json!({"data":[{"code":"202607MM","name":"2026年7月","values":[{"_id":"02d75ee002764e2ea53f263e52109e8f","i_showname":"居民消费价格指数 (上年同月=100) ","du_name":"%","catalogid":"b5e122302ad745358ba7415bd4a22c2f","value":"100.2","da":"110000000000","da_name":"北京市"}]}],"success":true,"state":20000})
            }
            _ => panic!("unexpected regional NBS fixture request"),
        };
        *call += 1;
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            serde_json::to_vec(&response).unwrap(),
        ))
    }
}

impl HttpTransport for FormalFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let mut call = self.call.lock().unwrap();
        let response = match *call {
            0 => catalog(
                "69c574ab128a44e595cc0b24502b771b",
                "",
                "月度数据",
                2,
                false,
                "fc982599aa684be7969d7b90b1bd0e84",
            ),
            1 => catalog(
                "fc982599aa684be7969d7b90b1bd0e84",
                "fc982599aa684be7969d7b90b1bd0e84",
                "价格指数",
                3,
                false,
                "3c9c459384c74f578f3541b2198aac70",
            ),
            2 => catalog(
                "3c9c459384c74f578f3541b2198aac70",
                "3c9c459384c74f578f3541b2198aac70",
                "居民消费价格分类指数 (上年同月=100)",
                4,
                false,
                "5b434e4d5e634a39b27a95f8251e9aae",
            ),
            3 => catalog(
                "5b434e4d5e634a39b27a95f8251e9aae",
                "5b434e4d5e634a39b27a95f8251e9aae",
                "全国居民消费价格分类指数 (上年同月=100) (2026-)",
                5,
                true,
                "5c7452825c7c4dcba391db5ca7f335c5",
            ),
            4 => {
                serde_json::json!({"data":{"total":1,"list":[{"_id":"53180dfb9c14411ba4b762307c85920c","i_showname":"居民消费价格指数 (上年同月=100) ","du_name":"%","catalogid":"5c7452825c7c4dcba391db5ca7f335c5"}]},"success":true,"state":20000})
            }
            5 => {
                let body: Value = serde_json::from_slice(request.body()).unwrap();
                assert_eq!(body["cid"], "5c7452825c7c4dcba391db5ca7f335c5");
                assert_eq!(body["indicatorIds"][0], "53180dfb9c14411ba4b762307c85920c");
                assert_eq!(body["das"][0]["value"], "000000000000");
                assert_eq!(body["dts"][0], "202607MM");
                serde_json::json!({"data":[{"code":"202607MM","name":"2026年7月","values":[{"_id":"53180dfb9c14411ba4b762307c85920c","i_showname":"居民消费价格指数 (上年同月=100) ","du_name":"%","catalogid":"5c7452825c7c4dcba391db5ca7f335c5","value":"100.5","da":"000000000000","da_name":"全国"}]}],"success":true,"state":20000})
            }
            _ => panic!("unexpected NBS fixture request"),
        };
        *call += 1;
        Ok(HttpResponse::new(
            200,
            request.url(),
            Some("application/json".into()),
            serde_json::to_vec(&response).unwrap(),
        ))
    }
}

fn catalog(
    parent: &str,
    expected_parent: &str,
    name: &str,
    level: u32,
    leaf: bool,
    id: &str,
) -> Value {
    let tree_parent = if expected_parent.is_empty() {
        parent
    } else {
        expected_parent
    };
    serde_json::json!({"data":[{"_id":id,"name":name,"treeinfo_pid":tree_parent,"treeinfo_level":level,"isLeaf":leaf,"type":"catalog"}],"success":true,"state":20000})
}

#[test]
fn formal_path_discovers_every_identity_before_returning_one_row() {
    let fixture = Arc::new(FormalFixture {
        call: Mutex::new(0),
    });
    let client = NbsClient::with_transports(fixture.clone(), fixture.clone()).unwrap();
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Nbs, "national-cpi-yoy", "headline").unwrap()],
        EconomicPeriod::month(2026, 7).unwrap(),
        EconomicPeriod::month(2026, 7).unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    let batch = client.economic_series(&request).unwrap();
    assert_eq!(batch.records()[0].value().unwrap().get(), 100.5);
    assert_eq!(*fixture.call.lock().unwrap(), 6);
}

#[test]
fn regional_formal_path_verifies_area_catalog_and_beijing_identity() {
    let fixture = Arc::new(RegionalFixture {
        call: Mutex::new(0),
    });
    let client = NbsClient::with_transports(fixture.clone(), fixture.clone()).unwrap();
    let request = EconomicSeriesRequest::new(
        vec![EconomicSeriesKey::new(ProviderId::Nbs, "beijing-cpi-yoy", "headline").unwrap()],
        EconomicPeriod::month(2026, 7).unwrap(),
        EconomicPeriod::month(2026, 7).unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    let batch = client.economic_series(&request).unwrap();
    assert_eq!(batch.records()[0].value().unwrap().get(), 100.2);
    assert_eq!(*fixture.call.lock().unwrap(), 8);
}
