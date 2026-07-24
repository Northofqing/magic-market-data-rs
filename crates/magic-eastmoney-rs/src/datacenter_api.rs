use crate::mapping::{optional_string, optional_u32};
use crate::{query_url, EastmoneyClient, EastmoneyError};
use serde_json::Value;

const ENDPOINT: &str = "https://datacenter-web.eastmoney.com/api/data/v1/get";
const PAGE_SIZE: u32 = 500;

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

struct DatacenterPage {
    rows: Vec<Value>,
    pages: u32,
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
    Ok(DatacenterPage { rows, pages })
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
#[path = "../tests/internal/datacenter_api_tests.rs"]
mod tests;
