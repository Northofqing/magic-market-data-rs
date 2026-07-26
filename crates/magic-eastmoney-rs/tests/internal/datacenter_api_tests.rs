use super::{decode_page, fetch_rows, instrument_filter};
use crate::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
use magic_market_core::IsoDate;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct PagingTransport {
    requests: Arc<Mutex<Vec<(u32, u32)>>>,
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
