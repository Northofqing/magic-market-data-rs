use crate::mapping::{optional_string, optional_u32};
use crate::{query_url, EastmoneyClient, EastmoneyError};
use serde_json::Value;

const ENDPOINT: &str = "https://datacenter-web.eastmoney.com/api/data/v1/get";
const PAGE_SIZE: u32 = 500;
const MAX_EXACT_PAGES: u32 = 20;

pub(crate) fn fetch_rows(
    client: &EastmoneyClient,
    report_name: &str,
    filter: &str,
    sort_column: &str,
    limit: u32,
) -> Result<Vec<Value>, EastmoneyError> {
    if limit == 0 || limit > 10_000 {
        return Err(EastmoneyError::InvalidRequest(
            "datacenter limit must be in 1..=10000".into(),
        ));
    }
    let mut output = Vec::new();
    let mut page = 1_u32;
    loop {
        let output_len = u32::try_from(output.len())
            .map_err(|_| EastmoneyError::Protocol("datacenter row count overflow".into()))?;
        let remaining = limit.saturating_sub(output_len);
        if remaining == 0 {
            break;
        }
        let url = query_url(
            ENDPOINT,
            &[
                ("reportName", report_name.into()),
                ("columns", "ALL".into()),
                ("filter", filter.into()),
                ("pageNumber", page.to_string()),
                // Keep the server-side page geometry stable. Changing this on
                // the final request changes the server offset for page N.
                ("pageSize", PAGE_SIZE.to_string()),
                ("sortColumns", sort_column.into()),
                ("sortTypes", "-1".into()),
                ("source", "WEB".into()),
                ("client", "WEB".into()),
            ],
        );
        let bytes = client.get(
            &url,
            &[
                ("Accept", "application/json"),
                ("Referer", "https://data.eastmoney.com/"),
            ],
        )?;
        let decoded = decode_page(&bytes)?;
        output.extend(decoded.rows);
        if page >= decoded.pages || output.len() >= limit as usize {
            break;
        }
        page = page
            .checked_add(1)
            .ok_or_else(|| EastmoneyError::Protocol("datacenter page counter overflow".into()))?;
    }
    output.truncate(limit as usize);
    Ok(output)
}

pub(crate) fn fetch_all_rows(
    client: &EastmoneyClient,
    report_name: &str,
    filter: &str,
    sort_column: &str,
    max_records: u32,
) -> Result<Vec<Value>, EastmoneyError> {
    if max_records == 0 || max_records > 10_000 {
        return Err(EastmoneyError::InvalidRequest(
            "exact datacenter maximum must be in 1..=10000".into(),
        ));
    }

    let mut output = Vec::new();
    let mut expected_pages = None;
    let mut expected_count = None;
    for page in 1..=MAX_EXACT_PAGES {
        let url = query_url(
            ENDPOINT,
            &[
                ("reportName", report_name.into()),
                ("columns", "ALL".into()),
                ("filter", filter.into()),
                ("pageNumber", page.to_string()),
                ("pageSize", PAGE_SIZE.to_string()),
                ("sortColumns", sort_column.into()),
                ("sortTypes", "-1".into()),
                ("source", "WEB".into()),
                ("client", "WEB".into()),
            ],
        );
        let bytes = client.get(
            &url,
            &[
                ("Accept", "application/json"),
                ("Referer", "https://data.eastmoney.com/"),
            ],
        )?;
        let decoded = decode_page(&bytes)?;

        if decoded.pages == 0 && decoded.rows.is_empty() && decoded.count == Some(0) {
            if page == 1 {
                return Ok(Vec::new());
            }
            return Err(EastmoneyError::Protocol(
                "datacenter changed from non-empty to empty during exact pagination".into(),
            ));
        }
        let count = decoded
            .count
            .ok_or_else(|| EastmoneyError::Protocol("datacenter result.count is absent".into()))?;
        if page == 1 {
            if decoded.pages == 0 {
                return Err(EastmoneyError::Protocol(
                    "non-empty datacenter result declares zero pages".into(),
                ));
            }
            if decoded.pages > MAX_EXACT_PAGES {
                return Err(EastmoneyError::Protocol(format!(
                    "exact datacenter result declares {} pages, above maximum {MAX_EXACT_PAGES}",
                    decoded.pages
                )));
            }
            if count > max_records {
                return Err(EastmoneyError::Protocol(format!(
                    "exact datacenter result declares {count} rows, above maximum {max_records}"
                )));
            }
            let computed_pages = count.div_ceil(PAGE_SIZE);
            if decoded.pages != computed_pages {
                return Err(EastmoneyError::Protocol(format!(
                    "datacenter page total {} contradicts count {count} at page size {PAGE_SIZE}",
                    decoded.pages
                )));
            }
            expected_pages = Some(decoded.pages);
            expected_count = Some(count);
        } else if Some(decoded.pages) != expected_pages || Some(count) != expected_count {
            return Err(EastmoneyError::Protocol(
                "datacenter page or record totals changed during exact pagination".into(),
            ));
        }

        let pages = expected_pages.expect("page one initializes exact totals");
        if decoded.rows.is_empty() {
            return Err(EastmoneyError::Protocol(format!(
                "datacenter exact page {page} of {pages} is empty"
            )));
        }
        output.extend(decoded.rows);
        if page == pages {
            let count = expected_count.expect("page one initializes exact totals");
            if output.len() != count as usize {
                return Err(EastmoneyError::Protocol(format!(
                    "datacenter exact coverage returned {} rows but declared {count}",
                    output.len()
                )));
            }
            return Ok(output);
        }
    }

    Err(EastmoneyError::Protocol(
        "datacenter exact pagination exceeded its page bound".into(),
    ))
}

struct DatacenterPage {
    rows: Vec<Value>,
    pages: u32,
    count: Option<u32>,
}

fn decode_page(bytes: &[u8]) -> Result<DatacenterPage, EastmoneyError> {
    let root: Value =
        serde_json::from_slice(bytes).map_err(|error| EastmoneyError::Decode(error.to_string()))?;
    if root.get("success").and_then(Value::as_bool) != Some(true) {
        let code = optional_string(root.get("code"))?.unwrap_or_else(|| "unknown".into());
        if code == "9201" {
            return Ok(DatacenterPage {
                rows: Vec::new(),
                pages: 0,
                count: Some(0),
            });
        }
        let message = optional_string(root.get("message"))?.unwrap_or_else(|| "no message".into());
        return Err(EastmoneyError::Protocol(format!(
            "datacenter returned code {code}: {message}"
        )));
    }
    let result = root
        .get("result")
        .ok_or_else(|| EastmoneyError::Protocol("datacenter result is absent".into()))?;
    let rows = result
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| EastmoneyError::Protocol("datacenter result.data is not an array".into()))?;
    let pages = optional_u32(result.get("pages"))?
        .ok_or_else(|| EastmoneyError::Protocol("datacenter result.pages is absent".into()))?;
    let count = optional_u32(result.get("count"))?;
    Ok(DatacenterPage { rows, pages, count })
}

pub(crate) fn instrument_filter(
    code_column: &str,
    code: &str,
    date_column: &str,
    start: Option<&magic_market_core::IsoDate>,
    end: Option<&magic_market_core::IsoDate>,
) -> String {
    let mut filter = format!("({code_column}=\"{code}\")");
    if let (Some(start), Some(end)) = (start, end) {
        filter.push_str(&format!(
            "({date_column}>='{}')({date_column}<='{}')",
            start.as_str(),
            end.as_str()
        ));
    }
    filter
}

#[cfg(test)]
mod tests {
    use super::{decode_page, fetch_all_rows, fetch_rows, instrument_filter};
    use crate::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
    use magic_market_core::IsoDate;
    use serde_json::{json, Value};
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
            decode_page(br#"{"success":true,"code":0,"result":{"data":[{"x":1}],"pages":2}}"#)
                .unwrap();
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.pages, 2);
        let empty =
            decode_page(r#"{"success":false,"code":9201,"message":"返回数据为空"}"#.as_bytes())
                .unwrap();
        assert!(empty.rows.is_empty());
        assert_eq!(empty.pages, 0);
        assert_eq!(empty.count, Some(0));
        assert!(
            decode_page(br#"{"success":false,"code":9501,"message":"filter invalid"}"#).is_err()
        );
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
}
