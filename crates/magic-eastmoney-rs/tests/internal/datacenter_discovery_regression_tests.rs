use super::{decode_page, fetch_all_rows, fetch_rows, instrument_filter};
use crate::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
use magic_market_core::IsoDate;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct PagingTransport {
    requests: Arc<Mutex<Vec<(u32, u32)>>>,
}

#[derive(Clone)]
struct ExactPagingTransport {
    count: u32,
    change_total_on_page_two: bool,
    empty_page_two: bool,
}

#[derive(Clone)]
struct SequenceTransport {
    bodies: Arc<Mutex<VecDeque<Vec<u8>>>>,
}

impl SequenceTransport {
    fn new(values: Vec<Value>) -> Self {
        Self {
            bodies: Arc::new(Mutex::new(
                values
                    .into_iter()
                    .map(|value| serde_json::to_vec(&value).unwrap())
                    .collect(),
            )),
        }
    }
}

impl EastmoneyTransport for SequenceTransport {
    fn get(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.bodies
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| EastmoneyError::Transport("sequence fixture exhausted".into()))
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Err(EastmoneyError::Unsupported(
            "sequence fixture does not use POST".into(),
        ))
    }
}

impl ExactPagingTransport {
    fn stable(count: u32) -> Self {
        Self {
            count,
            change_total_on_page_two: false,
            empty_page_two: false,
        }
    }

    fn changed_total() -> Self {
        Self {
            count: 1_001,
            change_total_on_page_two: true,
            empty_page_two: false,
        }
    }

    fn missing_page() -> Self {
        Self {
            count: 1_001,
            change_total_on_page_two: false,
            empty_page_two: true,
        }
    }
}

impl EastmoneyTransport for ExactPagingTransport {
    fn get(
        &self,
        url: &str,
        _headers: &[(&str, &str)],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        let page = query_u32(url, "pageNumber");
        let count = if self.change_total_on_page_two && page == 2 {
            self.count + 1
        } else {
            self.count
        };
        let start = (page - 1) * 500;
        let rows = if self.empty_page_two && page == 2 {
            Vec::new()
        } else {
            (start..(start + 500).min(self.count))
                .map(|id| json!({"id": id}))
                .collect::<Vec<_>>()
        };
        serde_json::to_vec(&json!({
            "success": true,
            "code": 0,
            "result": {
                "data": rows,
                "pages": count.div_ceil(500),
                "count": count
            }
        }))
        .map_err(|error| EastmoneyError::Decode(error.to_string()))
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Err(EastmoneyError::InvalidRequest(
            "exact paging fixture does not accept POST".into(),
        ))
    }
}

impl EastmoneyTransport for PagingTransport {
    fn get(
        &self,
        url: &str,
        _headers: &[(&str, &str)],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        let page = query_u32(url, "pageNumber");
        let page_size = query_u32(url, "pageSize");
        self.requests.lock().unwrap().push((page, page_size));
        let start = (page - 1) * page_size;
        let rows = (start..(start + page_size).min(1_000))
            .map(|id| json!({"id": id}))
            .collect::<Vec<_>>();
        serde_json::to_vec(&json!({
            "success": true,
            "code": 0,
            "result": {
                "data": rows,
                "pages": 1_000_u32.div_ceil(page_size)
            }
        }))
        .map_err(|error| EastmoneyError::Decode(error.to_string()))
    }

    fn post_json(
        &self,
        _url: &str,
        _headers: &[(&str, &str)],
        _body: &[u8],
        _max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        Err(EastmoneyError::InvalidRequest(
            "paging fixture does not accept POST".into(),
        ))
    }
}

fn query_u32(url: &str, key: &str) -> u32 {
    url.split_once('?')
        .unwrap()
        .1
        .split('&')
        .find_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            (name == key).then(|| value.parse::<u32>().unwrap())
        })
        .unwrap()
}

#[test]
fn decodes_success_and_documented_empty_status() {
    let page =
        decode_page(br#"{"success":true,"code":0,"result":{"data":[{"x":1}],"pages":2}}"#).unwrap();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.pages, 2);
    let empty = decode_page(r#"{"success":false,"code":9201,"message":"返回数据为空"}"#.as_bytes())
        .unwrap();
    assert!(empty.rows.is_empty());
    assert_eq!(empty.pages, 0);
    assert_eq!(empty.count, Some(0));
    assert!(decode_page(br#"{"success":false,"code":9501,"message":"filter invalid"}"#).is_err());
}

#[test]
fn filter_keeps_verified_quote_conventions() {
    let start = IsoDate::new("2026-01-01").unwrap();
    let end = IsoDate::new("2026-07-23").unwrap();
    assert_eq!(
        instrument_filter(
            "SECURITY_CODE",
            "600396",
            "TRADE_DATE",
            Some(&start),
            Some(&end)
        ),
        "(SECURITY_CODE=\"600396\")(TRADE_DATE>='2026-01-01')(TRADE_DATE<='2026-07-23')"
    );
}

#[test]
fn limit_above_remote_page_size_keeps_offsets_stable_and_truncates_locally() {
    let transport = PagingTransport::default();
    let requests = Arc::clone(&transport.requests);
    let client = EastmoneyClient::with_transport(transport);
    let rows = fetch_rows(&client, "REPORT", "", "DATE", 700).unwrap();
    let ids = rows
        .iter()
        .map(|row| row.get("id").and_then(Value::as_u64).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ids, (0_u64..700).collect::<Vec<_>>());
    assert_eq!(*requests.lock().unwrap(), vec![(1, 500), (2, 500)]);
}

#[test]
fn exact_reader_requires_stable_totals_and_full_coverage() {
    let client = EastmoneyClient::with_transport(ExactPagingTransport::stable(1_001));
    let rows = fetch_all_rows(
        &client,
        "RPT_DAILYBILLBOARD_DETAILSNEW",
        "(TRADE_DATE='2026-07-24')",
        "TRADE_ID",
        10_000,
    )
    .unwrap();
    assert_eq!(rows.len(), 1_001);

    let changed = EastmoneyClient::with_transport(ExactPagingTransport::changed_total());
    assert!(matches!(
        fetch_all_rows(
            &changed,
            "RPT_DAILYBILLBOARD_DETAILSNEW",
            "(TRADE_DATE='2026-07-24')",
            "TRADE_ID",
            10_000,
        ),
        Err(EastmoneyError::Protocol(_))
    ));

    let missing = EastmoneyClient::with_transport(ExactPagingTransport::missing_page());
    assert!(matches!(
        fetch_all_rows(
            &missing,
            "RPT_DAILYBILLBOARD_DETAILSNEW",
            "(TRADE_DATE='2026-07-24')",
            "TRADE_ID",
            10_000,
        ),
        Err(EastmoneyError::Protocol(_))
    ));
}

#[test]
fn datacenter_limits_filters_and_page_schema_fail_closed() {
    let client = EastmoneyClient::with_transport(PagingTransport::default());
    for limit in [0, 10_001] {
        assert!(fetch_rows(&client, "REPORT", "", "DATE", limit).is_err());
        assert!(fetch_all_rows(&client, "REPORT", "", "DATE", limit).is_err());
    }
    assert_eq!(
        instrument_filter("CODE", "600396", "DATE", None, None),
        "(CODE=\"600396\")"
    );

    for bytes in [
        b"{invalid".as_slice(),
        br#"{"success":true}"#,
        br#"{"success":true,"result":{}}"#,
        br#"{"success":true,"result":{"data":{},"pages":1}}"#,
        br#"{"success":true,"result":{"data":[]}}"#,
        br#"{"success":true,"result":{"data":[],"pages":"bad"}}"#,
        br#"{"success":false}"#,
    ] {
        assert!(decode_page(bytes).is_err());
    }
}

#[test]
fn exact_datacenter_rejects_every_declared_coverage_contradiction() {
    let cases = [
        vec![json!({
            "success": true,
            "result": {"data": [{"id": 1}], "pages": 0, "count": 1}
        })],
        vec![json!({
            "success": true,
            "result": {"data": [{"id": 1}], "pages": 21, "count": 1}
        })],
        vec![json!({
            "success": true,
            "result": {"data": [{"id": 1}], "pages": 1, "count": 2}
        })],
        vec![json!({
            "success": true,
            "result": {"data": [{"id": 1}], "pages": 1}
        })],
        vec![json!({
            "success": true,
            "result": {"data": [{"id": 1}], "pages": 1, "count": 20}
        })],
        vec![
            json!({
                "success": true,
                "result": {
                    "data": (0..500).map(|id| json!({"id": id})).collect::<Vec<_>>(),
                    "pages": 2,
                    "count": 501
                }
            }),
            json!({"success":false,"code":9201}),
        ],
    ];

    for bodies in cases {
        let client = EastmoneyClient::with_transport(SequenceTransport::new(bodies));
        assert!(matches!(
            fetch_all_rows(&client, "REPORT", "", "DATE", 10_000),
            Err(EastmoneyError::Protocol(_))
        ));
    }

    let above_caller_max = EastmoneyClient::with_transport(SequenceTransport::new(vec![json!({
        "success": true,
        "result": {"data": [{"id": 1}], "pages": 1, "count": 2}
    })]));
    assert!(fetch_all_rows(&above_caller_max, "REPORT", "", "DATE", 1).is_err());
}
