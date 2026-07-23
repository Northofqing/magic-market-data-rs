use crate::{now, valid_date, valid_time, SinaClient, SinaError};
use encoding_rs::GB18030;
use magic_market_core::{
    AssetClass, ContractMonth, DataBatch, Exchange, FiniteNumber, InstrumentId, Money,
    NonEmptyText, OptionCapabilities, OptionContract, OptionData, OptionGreeks, OptionKind,
    OptionQuote, Price, ProviderId, Quantity, Ratio, RatioUnit, SourceEvidence,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

const OPTION_REFERER: &str = "https://stock.finance.sina.com.cn/";
const OPTION_MONTH_ENDPOINT: &str =
    "https://stock.finance.sina.com.cn/futures/api/openapi.php/StockOptionService.getStockName";
const OPTION_QUOTE_ENDPOINT: &str = "https://hq.sinajs.cn/list=";
const MAX_MONTH_RESPONSE_ENTRIES: usize = 13;
const MAX_CONTRACT_MONTHS: usize = 12;
const MAX_CONTRACTS_PER_LIST: usize = 256;
const MAX_DISCOVERED_CONTRACTS: usize = 4_096;
const MAX_OPTION_BATCH_SIZE: usize = 50;
const MAX_HQ_RECORD_FIELDS: usize = MAX_CONTRACTS_PER_LIST + 1;
const MAX_OPTION_QUOTE_FIELDS: usize = 64;
const MAX_OPTION_GREEK_FIELDS: usize = 32;

#[derive(Debug, Deserialize)]
struct MonthRoot {
    result: MonthResult,
}

#[derive(Debug, Deserialize)]
struct MonthResult {
    status: MonthStatus,
    data: MonthData,
}

#[derive(Debug, Deserialize)]
struct MonthStatus {
    code: i64,
}

#[derive(Debug, Deserialize)]
struct MonthData {
    #[serde(rename = "contractMonth")]
    contract_months: Vec<String>,
}

fn underlying_category(underlying: &InstrumentId) -> Result<&'static str, SinaError> {
    if underlying.exchange() != Exchange::Shanghai || underlying.asset_class() != AssetClass::Fund {
        return Err(SinaError::Unsupported(format!(
            "Sina ETF options require a Shanghai fund underlying, got {:?} {:?}",
            underlying.exchange(),
            underlying.asset_class()
        )));
    }
    match underlying.code() {
        "510050" => Ok("50ETF"),
        "510300" => Ok("300ETF"),
        "588000" => Ok("%E7%A7%91%E5%88%9B50ETF"),
        "510500" => Ok("500ETF"),
        code => Err(SinaError::Unsupported(format!(
            "Sina ETF options do not support underlying {code}"
        ))),
    }
}

fn month_url(category: &str) -> String {
    format!("{OPTION_MONTH_ENDPOINT}?exchange=null&cate={category}")
}

fn parse_months(bytes: &[u8]) -> Result<Vec<ContractMonth>, SinaError> {
    let root: MonthRoot = serde_json::from_slice(bytes)
        .map_err(|error| SinaError::Decode(format!("option month JSON: {error}")))?;
    if root.result.status.code != 0 {
        return Err(SinaError::Protocol(format!(
            "option month endpoint returned status code {}",
            root.result.status.code
        )));
    }
    let source = root.result.data.contract_months;
    if source.len() < 2 {
        return Err(SinaError::Protocol(
            "option month response must contain the current-month marker and a month".into(),
        ));
    }
    if source.len() > MAX_MONTH_RESPONSE_ENTRIES {
        return Err(SinaError::Protocol(format!(
            "option month response has {} entries; limit is {MAX_MONTH_RESPONSE_ENTRIES}",
            source.len()
        )));
    }
    if source[0] != source[1] {
        return Err(SinaError::Protocol(format!(
            "option month marker {:?} does not repeat the current month {:?}",
            source[0], source[1]
        )));
    }

    let mut seen = HashSet::with_capacity(source.len() - 1);
    let mut months = Vec::with_capacity(source.len() - 1);
    for value in source.into_iter().skip(1) {
        let month = ContractMonth::new(value)?;
        if !seen.insert(month.clone()) {
            return Err(SinaError::Protocol(format!(
                "duplicate option contract month {}",
                month.as_str()
            )));
        }
        months.push(month);
    }
    if months.len() > MAX_CONTRACT_MONTHS {
        return Err(SinaError::Protocol(format!(
            "option month response has {} usable months; limit is {MAX_CONTRACT_MONTHS}",
            months.len()
        )));
    }
    Ok(months)
}

fn compact_month(month: &ContractMonth) -> String {
    format!("{}{}", &month.as_str()[2..4], &month.as_str()[5..7])
}

fn validate_contract_code(code: &str) -> Result<(), SinaError> {
    if code.len() != 8 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(SinaError::InvalidRequest(format!(
            "Sina option contract code {code:?} must contain exactly eight digits"
        )));
    }
    Ok(())
}

fn validate_contract_request(contracts: &[NonEmptyText]) -> Result<Vec<String>, SinaError> {
    if contracts.is_empty() {
        return Err(SinaError::InvalidRequest(
            "option contract list must not be empty".into(),
        ));
    }
    if contracts.len() > MAX_OPTION_BATCH_SIZE {
        return Err(SinaError::InvalidRequest(format!(
            "Sina option requests accept at most {MAX_OPTION_BATCH_SIZE} contracts"
        )));
    }
    let mut seen = HashSet::with_capacity(contracts.len());
    contracts
        .iter()
        .map(|contract| {
            let code = contract.as_str();
            validate_contract_code(code)?;
            if !seen.insert(code) {
                return Err(SinaError::InvalidRequest(format!(
                    "duplicate option contract code {code}"
                )));
            }
            Ok(code.to_owned())
        })
        .collect()
}

fn decode_hq_response(bytes: &[u8]) -> Result<HashMap<String, Vec<String>>, SinaError> {
    if bytes.is_empty() {
        return Err(SinaError::Protocol("empty Sina option response".into()));
    }
    let (decoded, _, had_errors) = GB18030.decode(bytes);
    if had_errors {
        return Err(SinaError::Decode(
            "option response contains invalid GB18030 byte sequences".into(),
        ));
    }
    let mut records = HashMap::new();
    for line in decoded
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let line = line
            .strip_prefix("var hq_str_")
            .ok_or_else(|| SinaError::Protocol(format!("invalid option response shell: {line}")))?;
        let (key, payload) = line.split_once("=\"").ok_or_else(|| {
            SinaError::Protocol(format!(
                "option response omits assignment delimiter: {line}"
            ))
        })?;
        let payload = payload.strip_suffix("\";").ok_or_else(|| {
            SinaError::Protocol(format!("option response omits closing shell: {line}"))
        })?;
        if key.is_empty()
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(SinaError::Protocol(format!(
                "invalid option response key {key:?}"
            )));
        }
        let fields = payload.split(',').map(str::to_owned).collect::<Vec<_>>();
        if fields.len() > MAX_HQ_RECORD_FIELDS {
            return Err(SinaError::Protocol(format!(
                "option response {key} has {} fields; limit is {MAX_HQ_RECORD_FIELDS}",
                fields.len()
            )));
        }
        if records.insert(key.to_owned(), fields).is_some() {
            return Err(SinaError::Protocol(format!(
                "duplicate option response record {key}"
            )));
        }
    }
    if records.is_empty() {
        return Err(SinaError::Protocol(
            "option response contained no records".into(),
        ));
    }
    Ok(records)
}

fn one_hq_record(bytes: &[u8], expected_key: &str) -> Result<Vec<String>, SinaError> {
    let mut records = decode_hq_response(bytes)?;
    let fields = records.remove(expected_key).ok_or_else(|| {
        SinaError::Protocol(format!(
            "option response omitted requested record {expected_key}"
        ))
    })?;
    if !records.is_empty() {
        return Err(SinaError::Protocol(
            "option response contained an unexpected record".into(),
        ));
    }
    Ok(fields)
}

fn parse_contract_codes(bytes: &[u8], key: &str) -> Result<Vec<String>, SinaError> {
    let fields = one_hq_record(bytes, key)?;
    let mut codes = Vec::new();
    let mut seen = HashSet::new();
    let field_count = fields.len();
    for (index, field) in fields.into_iter().enumerate() {
        let value = field.trim();
        if value.is_empty() {
            if index + 1 == field_count {
                continue;
            }
            return Err(SinaError::Protocol(format!(
                "option contract list {key} contains an empty interior field"
            )));
        }
        let code = value.strip_prefix("CON_OP_").ok_or_else(|| {
            SinaError::Protocol(format!(
                "option contract list {key} contains invalid entry {value:?}"
            ))
        })?;
        validate_contract_code(code).map_err(|error| SinaError::Protocol(error.to_string()))?;
        if !seen.insert(code.to_owned()) {
            return Err(SinaError::Protocol(format!(
                "option contract list {key} repeats contract {code}"
            )));
        }
        codes.push(code.to_owned());
    }
    if codes.is_empty() {
        return Err(SinaError::Protocol(format!(
            "option contract list {key} is empty"
        )));
    }
    if codes.len() > MAX_CONTRACTS_PER_LIST {
        return Err(SinaError::Protocol(format!(
            "option contract list {key} has {} entries; limit is {MAX_CONTRACTS_PER_LIST}",
            codes.len()
        )));
    }
    Ok(codes)
}

fn optional_number(value: &str, field: &str) -> Result<Option<f64>, SinaError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let parsed = value.parse::<f64>().map_err(|_| {
        SinaError::Protocol(format!("option field {field} is not numeric: {value:?}"))
    })?;
    if !parsed.is_finite() {
        return Err(SinaError::Protocol(format!(
            "option field {field} must be finite"
        )));
    }
    Ok(Some(parsed))
}

fn optional_price(value: &str, field: &str) -> Result<Option<Price>, SinaError> {
    match optional_number(value, field)? {
        None | Some(0.0) => Ok(None),
        Some(value) => Ok(Some(Price::new(value)?)),
    }
}

fn optional_quantity(value: &str, field: &str) -> Result<Option<Quantity>, SinaError> {
    optional_number(value, field)?
        .map(Quantity::new)
        .transpose()
        .map_err(Into::into)
}

fn optional_level(
    price_value: &str,
    quantity_value: &str,
    price_field: &str,
    quantity_field: &str,
) -> Result<(Option<Price>, Option<Quantity>), SinaError> {
    let price = optional_price(price_value, price_field)?;
    let quantity = optional_quantity(quantity_value, quantity_field)?;
    match (price, quantity) {
        (None, None) => Ok((None, None)),
        (None, Some(quantity)) if quantity.get() == 0.0 => Ok((None, None)),
        (Some(price), Some(quantity)) => Ok((Some(price), Some(quantity))),
        _ => Err(SinaError::Protocol(format!(
            "option {price_field}/{quantity_field} must be present together"
        ))),
    }
}

fn optional_money(value: &str, field: &str) -> Result<Option<Money>, SinaError> {
    match optional_number(value, field)? {
        Some(value) if value < 0.0 => Err(SinaError::Protocol(format!(
            "option field {field} must be non-negative"
        ))),
        Some(value) => Ok(Some(Money::new(value)?)),
        None => Ok(None),
    }
}

fn optional_finite(value: &str, field: &str) -> Result<Option<FiniteNumber>, SinaError> {
    optional_number(value, field)?
        .map(FiniteNumber::new)
        .transpose()
        .map_err(Into::into)
}

fn optional_non_negative_finite(
    value: &str,
    field: &str,
) -> Result<Option<FiniteNumber>, SinaError> {
    match optional_number(value, field)? {
        Some(value) if value < 0.0 => Err(SinaError::Protocol(format!(
            "option field {field} must be non-negative"
        ))),
        Some(value) => Ok(Some(FiniteNumber::new(value)?)),
        None => Ok(None),
    }
}

fn optional_delta(value: &str) -> Result<Option<FiniteNumber>, SinaError> {
    match optional_number(value, "delta")? {
        Some(value) if !(-1.0..=1.0).contains(&value) => Err(SinaError::Protocol(
            "option field delta must be between -1 and 1".into(),
        )),
        Some(value) => Ok(Some(FiniteNumber::new(value)?)),
        None => Ok(None),
    }
}

fn optional_ratio(value: &str, field: &str, unit: RatioUnit) -> Result<Option<Ratio>, SinaError> {
    optional_number(value, field)?
        .map(|value| Ratio::new(value, unit))
        .transpose()
        .map_err(Into::into)
}

fn optional_text(value: &str) -> Result<Option<NonEmptyText>, SinaError> {
    if value.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(NonEmptyText::new(value)?))
    }
}

fn optional_quote_timestamp(value: &str) -> Result<Option<NonEmptyText>, SinaError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() != 19
        || value.as_bytes().get(10) != Some(&b' ')
        || !valid_date(&value[..10])
        || !valid_time(&value[11..])
    {
        return Err(SinaError::Protocol(format!(
            "option quote timestamp must use a valid YYYY-MM-DD HH:MM:SS value: {value:?}"
        )));
    }
    Ok(Some(NonEmptyText::new(format!(
        "{}T{}+08:00",
        &value[..10],
        &value[11..]
    ))?))
}

fn validate_quote_bounds(
    contract_code: &str,
    bid: Option<Price>,
    ask: Option<Price>,
    high: Option<Price>,
    low: Option<Price>,
    upper_limit: Option<Price>,
    lower_limit: Option<Price>,
) -> Result<(), SinaError> {
    for (left_name, left, relation, right_name, right) in [
        ("bid", bid, "<=", "ask", ask),
        ("low", low, "<=", "high", high),
        ("lower_limit", lower_limit, "<=", "upper_limit", upper_limit),
    ] {
        if let (Some(left), Some(right)) = (left, right) {
            if left.get() > right.get() {
                return Err(SinaError::Protocol(format!(
                    "option quote {contract_code} violates {left_name} {relation} {right_name}"
                )));
            }
        }
    }
    Ok(())
}

fn parse_quote(
    contract_code: &str,
    fields: &[String],
    observed_at: &str,
    batch_id: &str,
) -> Result<OptionQuote, SinaError> {
    if fields.len() < 43 || fields.len() > MAX_OPTION_QUOTE_FIELDS {
        return Err(SinaError::Protocol(format!(
            "option quote {contract_code} has {} fields; expected 43..={MAX_OPTION_QUOTE_FIELDS}",
            fields.len(),
        )));
    }
    let quote_at = optional_quote_timestamp(&fields[32])?;
    let (bid, bid_quantity) = optional_level(&fields[1], &fields[0], "bid", "bid_quantity")?;
    let (ask, ask_quantity) = optional_level(&fields[3], &fields[4], "ask", "ask_quantity")?;
    let high = optional_price(&fields[39], "high")?;
    let low = optional_price(&fields[40], "low")?;
    let upper_limit = optional_price(&fields[10], "upper_limit")?;
    let lower_limit = optional_price(&fields[11], "lower_limit")?;
    validate_quote_bounds(contract_code, bid, ask, high, low, upper_limit, lower_limit)?;
    let mut evidence = SourceEvidence::new(ProviderId::Sina, observed_at, batch_id.to_owned())?;
    if let Some(source_at) = quote_at.as_ref() {
        evidence = evidence.with_source_at(source_at.as_str())?;
    }
    let amplitude = optional_ratio(&fields[38], "amplitude", RatioUnit::Percent)?;
    if amplitude.is_some_and(|value| value.get() < 0.0) {
        return Err(SinaError::Protocol(
            "option field amplitude must be non-negative".into(),
        ));
    }
    Ok(OptionQuote {
        contract_code: NonEmptyText::new(contract_code)?,
        name: optional_text(&fields[37])?,
        bid,
        bid_quantity,
        ask,
        ask_quantity,
        last: optional_price(&fields[2], "last")?,
        previous_close: optional_price(&fields[8], "previous_close")?,
        open: optional_price(&fields[9], "open")?,
        high,
        low,
        upper_limit,
        lower_limit,
        strike: optional_price(&fields[7], "strike")?,
        volume: optional_quantity(&fields[41], "volume")?,
        open_interest: optional_quantity(&fields[5], "open_interest")?,
        amount: optional_money(&fields[42], "amount")?,
        change: optional_ratio(&fields[6], "change", RatioUnit::Percent)?,
        amplitude,
        quote_at,
        evidence,
    })
}

fn parse_greeks(
    contract_code: &str,
    fields: &[String],
    observed_at: &str,
    batch_id: &str,
) -> Result<OptionGreeks, SinaError> {
    if fields.len() < 16 || fields.len() > MAX_OPTION_GREEK_FIELDS {
        return Err(SinaError::Protocol(format!(
            "option greeks {contract_code} has {} fields; expected 16..={MAX_OPTION_GREEK_FIELDS}",
            fields.len(),
        )));
    }
    if fields[1..4].iter().any(|field| !field.is_empty()) {
        return Err(SinaError::Protocol(format!(
            "option greeks {contract_code} fields 1 through 3 must be exactly empty"
        )));
    }
    let high = optional_price(&fields[10], "high")?;
    let low = optional_price(&fields[11], "low")?;
    if let (Some(low), Some(high)) = (low, high) {
        if low.get() > high.get() {
            return Err(SinaError::Protocol(format!(
                "option greeks {contract_code} violates low <= high"
            )));
        }
    }
    Ok(OptionGreeks {
        contract_code: NonEmptyText::new(contract_code)?,
        name: optional_text(&fields[0])?,
        volume: optional_quantity(&fields[4], "volume")?,
        delta: optional_delta(&fields[5])?,
        gamma: optional_non_negative_finite(&fields[6], "gamma")?,
        theta: optional_finite(&fields[7], "theta")?,
        vega: optional_non_negative_finite(&fields[8], "vega")?,
        rho: None,
        implied_volatility: optional_non_negative_finite(&fields[9], "implied_volatility")?,
        high,
        low,
        trade_code: optional_text(&fields[12])?,
        strike: optional_price(&fields[13], "strike")?,
        last: optional_price(&fields[14], "last")?,
        theoretical_price: optional_price(&fields[15], "theoretical_price")?,
        evidence: SourceEvidence::new(ProviderId::Sina, observed_at, batch_id.to_owned())?,
    })
}

fn parse_requested_records<T>(
    bytes: &[u8],
    contracts: &[String],
    prefix: &str,
    observed_at: &str,
    batch_id: &str,
    parser: impl Fn(&str, &[String], &str, &str) -> Result<T, SinaError>,
) -> Result<Vec<T>, SinaError> {
    let mut records = decode_hq_response(bytes)?;
    let mut parsed = Vec::with_capacity(contracts.len());
    for contract in contracts {
        let key = format!("{prefix}{contract}");
        let fields = records.remove(&key).ok_or_else(|| {
            SinaError::Protocol(format!("option response omitted requested record {key}"))
        })?;
        parsed.push(parser(contract, &fields, observed_at, batch_id)?);
    }
    if !records.is_empty() {
        return Err(SinaError::Protocol(
            "option response contained an unexpected record".into(),
        ));
    }
    Ok(parsed)
}

impl SinaClient {
    pub const fn option_capabilities() -> OptionCapabilities {
        OptionCapabilities {
            contract_discovery: true,
            quotes: true,
            greeks: true,
        }
    }
}

impl OptionData for SinaClient {
    type Error = SinaError;

    fn option_contracts(
        &self,
        underlying: &InstrumentId,
        expiry: Option<&ContractMonth>,
    ) -> Result<DataBatch<OptionContract>, Self::Error> {
        let category = underlying_category(underlying)?;
        let observed_at = now()?;
        let month_bytes = self
            .transport
            .get_with_referer(&month_url(category), OPTION_REFERER)?;
        let discovered = parse_months(&month_bytes)?;
        let months = if let Some(expiry) = expiry {
            if !discovered.contains(expiry) {
                return Err(SinaError::Protocol(format!(
                    "requested option month {} is not listed by Sina",
                    expiry.as_str()
                )));
            }
            vec![expiry.clone()]
        } else {
            discovered
        };
        let batch_id = format!("sina-web:{observed_at}:option-contracts");
        let mut records = Vec::new();
        let mut seen = HashSet::new();
        for month in months {
            let compact = compact_month(&month);
            for (kind, side) in [(OptionKind::Call, "OP_UP_"), (OptionKind::Put, "OP_DOWN_")] {
                let key = format!("{side}{}{compact}", underlying.code());
                let bytes = self
                    .transport
                    .get_with_referer(&format!("{OPTION_QUOTE_ENDPOINT}{key}"), OPTION_REFERER)?;
                for contract_code in parse_contract_codes(&bytes, &key)? {
                    if !seen.insert(contract_code.clone()) {
                        return Err(SinaError::Protocol(format!(
                            "option discovery repeats contract {contract_code}"
                        )));
                    }
                    records.push(OptionContract {
                        contract_code: NonEmptyText::new(contract_code)?,
                        underlying: underlying.clone(),
                        expiry_month: month.clone(),
                        expiry: None,
                        kind,
                        strike: None,
                        evidence: SourceEvidence::new(
                            ProviderId::Sina,
                            &observed_at,
                            batch_id.clone(),
                        )?,
                    });
                    if records.len() > MAX_DISCOVERED_CONTRACTS {
                        return Err(SinaError::Protocol(format!(
                            "option discovery exceeds {MAX_DISCOVERED_CONTRACTS} contracts"
                        )));
                    }
                }
            }
        }
        if records.is_empty() {
            return Err(SinaError::Protocol(
                "option discovery produced no contracts".into(),
            ));
        }
        let provenance = magic_market_core::Provenance::new("sina-web", &observed_at)?
            .with_batch_id(batch_id)?;
        Ok(DataBatch::strict(records, provenance))
    }

    fn option_quotes(
        &self,
        contracts: &[NonEmptyText],
    ) -> Result<DataBatch<OptionQuote>, Self::Error> {
        let contracts = validate_contract_request(contracts)?;
        let observed_at = now()?;
        let batch_id = format!("sina-web:{observed_at}:option-quotes");
        let query = contracts
            .iter()
            .map(|contract| format!("CON_OP_{contract}"))
            .collect::<Vec<_>>()
            .join(",");
        let bytes = self
            .transport
            .get_with_referer(&format!("{OPTION_QUOTE_ENDPOINT}{query}"), OPTION_REFERER)?;
        let records = parse_requested_records(
            &bytes,
            &contracts,
            "CON_OP_",
            &observed_at,
            &batch_id,
            parse_quote,
        )?;
        let provenance = magic_market_core::Provenance::new("sina-web", &observed_at)?
            .with_batch_id(batch_id)?;
        Ok(DataBatch::strict(records, provenance))
    }

    fn option_greeks(
        &self,
        contracts: &[NonEmptyText],
    ) -> Result<DataBatch<OptionGreeks>, Self::Error> {
        let contracts = validate_contract_request(contracts)?;
        let observed_at = now()?;
        let batch_id = format!("sina-web:{observed_at}:option-greeks");
        let query = contracts
            .iter()
            .map(|contract| format!("CON_SO_{contract}"))
            .collect::<Vec<_>>()
            .join(",");
        let bytes = self
            .transport
            .get_with_referer(&format!("{OPTION_QUOTE_ENDPOINT}{query}"), OPTION_REFERER)?;
        let records = parse_requested_records(
            &bytes,
            &contracts,
            "CON_SO_",
            &observed_at,
            &batch_id,
            parse_greeks,
        )?;
        let provenance = magic_market_core::Provenance::new("sina-web", &observed_at)?
            .with_batch_id(batch_id)?;
        Ok(DataBatch::strict(records, provenance))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SnapshotTransport;
    use magic_market_core::{OptionData, RatioUnit};
    use std::sync::{Arc, Mutex};

    const MONTH_FIXTURE: &str = r#"{
      "result": {
        "status": {"code": 0},
        "data": {
          "contractMonth": ["2026-08", "2026-08", "2026-09", "2026-12", "2027-03"]
        }
      }
    }"#;
    const CALL_LIST: &str = r#"var hq_str_OP_UP_5100502608="CON_OP_10012127,CON_OP_10011851,";"#;
    const PUT_LIST: &str = r#"var hq_str_OP_DOWN_5100502608="CON_OP_10012128,CON_OP_10011861,";"#;
    const QUOTE_FIXTURE: &str = concat!(
        "var hq_str_CON_OP_10012127=\"",
        "1,0.3241,0.3268,0.3273,2,396,-2.74,2.7500,0.3281,0.3292,0.6446,0.0274,",
        "0.3330,16,0.3328,1,0.3292,1,0.3278,1,0.3273,1,0.3241,1,0.3218,1,",
        "0.3172,1,0.3171,1,0.3155,1,2026-07-23 14:48:38,0,E 00,EBS,510050,",
        "50ETF购8月2750,6.77,0.3375,0.3153,89,289374.00,M,0.3360,C,2026-08-26,",
        "34,2,0.334,-0.0072\";"
    );
    const GREEKS_FIXTURE: &str = concat!(
        "var hq_str_CON_SO_10012127=\"",
        "50ETF购8月2750,,,,89,0.9718,0.332,-0.1734,0.0608,0.0008,0.3375,",
        "0.3153,510050C2608M02750,2.7500,0.3268,0.3464,M\";"
    );

    #[derive(Clone, Default)]
    struct FixtureTransport {
        responses: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        requests: Arc<Mutex<Vec<(String, String)>>>,
    }

    impl FixtureTransport {
        fn insert(&self, url: impl Into<String>, response: impl Into<Vec<u8>>) {
            self.responses
                .lock()
                .unwrap()
                .insert(url.into(), response.into());
        }
    }

    impl SnapshotTransport for FixtureTransport {
        fn get(&self, url: &str) -> Result<Vec<u8>, SinaError> {
            self.get_with_referer(url, "")
        }

        fn get_with_referer(&self, url: &str, referer: &str) -> Result<Vec<u8>, SinaError> {
            self.requests
                .lock()
                .unwrap()
                .push((url.to_owned(), referer.to_owned()));
            self.responses
                .lock()
                .unwrap()
                .get(url)
                .cloned()
                .ok_or_else(|| SinaError::Transport(format!("no fixture for {url}")))
        }
    }

    fn underlying(code: &str) -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, code, AssetClass::Fund).unwrap()
    }

    fn contract(code: &str) -> NonEmptyText {
        NonEmptyText::new(code).unwrap()
    }

    fn gb18030(value: &str) -> Vec<u8> {
        let (encoded, _, had_errors) = GB18030.encode(value);
        assert!(!had_errors);
        encoded.into_owned()
    }

    #[test]
    fn parses_month_marker_and_rejects_a_contradiction() {
        let months = parse_months(MONTH_FIXTURE.as_bytes()).unwrap();
        assert_eq!(months.len(), 4);
        assert_eq!(months[0].as_str(), "2026-08");

        let contradictory =
            MONTH_FIXTURE.replacen(r#""2026-08", "2026-08""#, r#""2026-07", "2026-08""#, 1);
        assert!(matches!(
            parse_months(contradictory.as_bytes()),
            Err(SinaError::Protocol(message)) if message.contains("does not repeat")
        ));
    }

    #[test]
    fn discovers_both_sides_for_an_explicit_month_with_evidence() {
        let transport = FixtureTransport::default();
        transport.insert(month_url("50ETF"), MONTH_FIXTURE.as_bytes().to_vec());
        transport.insert(
            format!("{OPTION_QUOTE_ENDPOINT}OP_UP_5100502608"),
            CALL_LIST.as_bytes().to_vec(),
        );
        transport.insert(
            format!("{OPTION_QUOTE_ENDPOINT}OP_DOWN_5100502608"),
            PUT_LIST.as_bytes().to_vec(),
        );
        let client = SinaClient::with_transport(transport.clone());
        let month = ContractMonth::new("2026-08").unwrap();
        let batch = client
            .option_contracts(&underlying("510050"), Some(&month))
            .unwrap();

        assert_eq!(batch.records().len(), 4);
        assert_eq!(batch.records()[0].kind, OptionKind::Call);
        assert_eq!(batch.records()[2].kind, OptionKind::Put);
        assert_eq!(batch.records()[0].evidence.provider(), ProviderId::Sina);
        assert_eq!(
            batch.records()[0].evidence.batch_id(),
            batch.provenance().batch_id().unwrap()
        );
        assert!(batch
            .records()
            .iter()
            .all(|record| record.expiry.is_none() && record.strike.is_none()));
        assert!(transport
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|(_, referer)| referer == OPTION_REFERER));
    }

    #[test]
    fn parses_complete_t_quote_and_source_timestamp() {
        let transport = FixtureTransport::default();
        let url = format!("{OPTION_QUOTE_ENDPOINT}CON_OP_10012127");
        transport.insert(url, gb18030(QUOTE_FIXTURE));
        let client = SinaClient::with_transport(transport);
        let batch = client.option_quotes(&[contract("10012127")]).unwrap();
        let quote = &batch.records()[0];

        assert_eq!(quote.bid_quantity.unwrap().get(), 1.0);
        assert_eq!(quote.ask_quantity.unwrap().get(), 2.0);
        assert_eq!(quote.last.unwrap().get(), 0.3268);
        assert_eq!(quote.strike.unwrap().get(), 2.75);
        assert_eq!(quote.open_interest.unwrap().get(), 396.0);
        assert_eq!(quote.amount.unwrap().get(), 289_374.0);
        assert_eq!(quote.change.unwrap().get(), -2.74);
        assert_eq!(quote.change.unwrap().unit(), RatioUnit::Percent);
        assert_eq!(quote.amplitude.unwrap().get(), 6.77);
        assert_eq!(
            quote.quote_at.as_ref().unwrap().as_str(),
            "2026-07-23T14:48:38+08:00"
        );
        assert_eq!(
            quote.evidence.source_at(),
            Some("2026-07-23T14:48:38+08:00")
        );
    }

    #[test]
    fn parses_greeks_after_exact_three_field_gap() {
        let transport = FixtureTransport::default();
        let url = format!("{OPTION_QUOTE_ENDPOINT}CON_SO_10012127");
        transport.insert(url, gb18030(GREEKS_FIXTURE));
        let client = SinaClient::with_transport(transport);
        let batch = client.option_greeks(&[contract("10012127")]).unwrap();
        let greeks = &batch.records()[0];

        assert_eq!(greeks.volume.unwrap().get(), 89.0);
        assert_eq!(greeks.delta.unwrap().get(), 0.9718);
        assert_eq!(greeks.gamma.unwrap().get(), 0.332);
        assert_eq!(greeks.theta.unwrap().get(), -0.1734);
        assert_eq!(greeks.vega.unwrap().get(), 0.0608);
        assert_eq!(greeks.implied_volatility.unwrap().get(), 0.0008);
        assert_eq!(
            greeks.trade_code.as_ref().unwrap().as_str(),
            "510050C2608M02750"
        );
        assert_eq!(greeks.theoretical_price.unwrap().get(), 0.3464);
        assert!(greeks.rho.is_none());
    }

    #[test]
    fn rejects_shifted_greek_fields_instead_of_mislabeling_them() {
        let shifted = GREEKS_FIXTURE.replacen(",,,,89", ",,bad,,89", 1);
        assert!(matches!(
            parse_greeks(
                "10012127",
                &one_hq_record(&gb18030(&shifted), "CON_SO_10012127").unwrap(),
                "observed",
                "batch"
            ),
            Err(SinaError::Protocol(message)) if message.contains("exactly empty")
        ));
    }

    #[test]
    fn rejects_invalid_greek_domains_and_inverted_range() {
        for (fixture, expected) in [
            (GREEKS_FIXTURE.replacen("0.9718", "1.0001", 1), "delta"),
            (GREEKS_FIXTURE.replacen("0.332", "-0.001", 1), "gamma"),
            (GREEKS_FIXTURE.replacen("0.0608", "-0.001", 1), "vega"),
            (
                GREEKS_FIXTURE.replacen("0.0008", "-0.001", 1),
                "implied_volatility",
            ),
            (
                GREEKS_FIXTURE.replacen("0.3375", "0.3000", 1),
                "low <= high",
            ),
        ] {
            let fields = one_hq_record(&gb18030(&fixture), "CON_SO_10012127").unwrap();
            assert!(matches!(
                parse_greeks("10012127", &fields, "observed", "batch"),
                Err(SinaError::Protocol(message)) if message.contains(expected)
            ));
        }
    }

    #[test]
    fn contract_list_limit_is_independent_from_quote_field_limit() {
        let entries = (0..129)
            .map(|index| format!("CON_OP_{:08}", 10_000_000 + index))
            .collect::<Vec<_>>()
            .join(",");
        let fixture = format!("var hq_str_OP_UP_5100502608=\"{entries},\";");
        assert_eq!(
            parse_contract_codes(fixture.as_bytes(), "OP_UP_5100502608")
                .unwrap()
                .len(),
            129
        );

        let too_many = (0..=MAX_CONTRACTS_PER_LIST)
            .map(|index| format!("CON_OP_{:08}", 10_000_000 + index))
            .collect::<Vec<_>>()
            .join(",");
        let fixture = format!("var hq_str_OP_UP_5100502608=\"{too_many}\";");
        assert!(matches!(
            parse_contract_codes(fixture.as_bytes(), "OP_UP_5100502608"),
            Err(SinaError::Protocol(message)) if message.contains("limit")
        ));
    }

    #[test]
    fn rejects_unsupported_underlying_and_bad_or_duplicate_contracts() {
        assert_eq!(underlying_category(&underlying("510050")).unwrap(), "50ETF");
        assert_eq!(
            underlying_category(&underlying("510300")).unwrap(),
            "300ETF"
        );
        assert_eq!(
            underlying_category(&underlying("588000")).unwrap(),
            "%E7%A7%91%E5%88%9B50ETF"
        );
        assert_eq!(
            underlying_category(&underlying("510500")).unwrap(),
            "500ETF"
        );
        assert!(matches!(
            underlying_category(&underlying("510880")),
            Err(SinaError::Unsupported(_))
        ));
        let client = SinaClient::with_transport(FixtureTransport::default());
        assert!(matches!(
            client.option_quotes(&[contract("bad")]),
            Err(SinaError::InvalidRequest(_))
        ));
        assert!(matches!(
            client.option_quotes(&[contract("10012127"), contract("10012127")]),
            Err(SinaError::InvalidRequest(message)) if message.contains("duplicate")
        ));
        let too_many = (0..=MAX_OPTION_BATCH_SIZE)
            .map(|index| contract(&format!("{:08}", 10_000_000 + index)))
            .collect::<Vec<_>>();
        assert!(matches!(
            client.option_quotes(&too_many),
            Err(SinaError::InvalidRequest(message)) if message.contains("at most")
        ));
    }

    #[test]
    fn rejects_invalid_numeric_payload_and_missing_requested_record() {
        let invalid = QUOTE_FIXTURE.replacen("0.3241", "not-a-number", 1);
        let fields = one_hq_record(&gb18030(&invalid), "CON_OP_10012127").unwrap();
        assert!(matches!(
            parse_quote("10012127", &fields, "observed", "batch"),
            Err(SinaError::Protocol(message)) if message.contains("not numeric")
        ));

        assert!(matches!(
            parse_requested_records(
                &gb18030(QUOTE_FIXTURE),
                &["10011851".to_owned()],
                "CON_OP_",
                "observed",
                "batch",
                parse_quote
            ),
            Err(SinaError::Protocol(message)) if message.contains("omitted requested")
        ));
    }

    #[test]
    fn rejects_invalid_timestamp_negative_amount_and_inverted_price_bounds() {
        let invalid_time = QUOTE_FIXTURE.replacen("2026-07-23 14:48:38", "2026-02-30 14:48:38", 1);
        let fields = one_hq_record(&gb18030(&invalid_time), "CON_OP_10012127").unwrap();
        assert!(matches!(
            parse_quote("10012127", &fields, "observed", "batch"),
            Err(SinaError::Protocol(message)) if message.contains("timestamp")
        ));

        let negative_amount = QUOTE_FIXTURE.replacen("289374.00", "-1.00", 1);
        let fields = one_hq_record(&gb18030(&negative_amount), "CON_OP_10012127").unwrap();
        assert!(matches!(
            parse_quote("10012127", &fields, "observed", "batch"),
            Err(SinaError::Protocol(message))
                if message.contains("amount") && message.contains("non-negative")
        ));

        for (fixture, expected_relation) in [
            (QUOTE_FIXTURE.replacen("0.3241", "0.4000", 1), "bid <= ask"),
            (QUOTE_FIXTURE.replacen("0.3375", "0.3000", 1), "low <= high"),
            (
                QUOTE_FIXTURE.replacen("0.6446", "0.0100", 1),
                "lower_limit <= upper_limit",
            ),
        ] {
            let fields = one_hq_record(&gb18030(&fixture), "CON_OP_10012127").unwrap();
            assert!(matches!(
                parse_quote("10012127", &fields, "observed", "batch"),
                Err(SinaError::Protocol(message)) if message.contains(expected_relation)
            ));
        }
    }

    #[test]
    fn normalizes_zero_book_level_and_rejects_half_levels_or_negative_amplitude() {
        let zero_bid = QUOTE_FIXTURE.replacen("1,0.3241", "0,0", 1);
        let fields = one_hq_record(&gb18030(&zero_bid), "CON_OP_10012127").unwrap();
        let quote = parse_quote("10012127", &fields, "observed", "batch").unwrap();
        assert!(quote.bid.is_none());
        assert!(quote.bid_quantity.is_none());
        let encoded = serde_json::to_string(&quote).unwrap();
        assert!(serde_json::from_str::<OptionQuote>(&encoded).is_ok());

        for (fixture, expected) in [
            (
                QUOTE_FIXTURE.replacen("1,0.3241", "1,0", 1),
                "present together",
            ),
            (
                QUOTE_FIXTURE.replacen("1,0.3241", ",0.3241", 1),
                "present together",
            ),
            (
                QUOTE_FIXTURE.replacen("50ETF购8月2750,6.77", "50ETF购8月2750,-1", 1),
                "amplitude",
            ),
        ] {
            let fields = one_hq_record(&gb18030(&fixture), "CON_OP_10012127").unwrap();
            assert!(matches!(
                parse_quote("10012127", &fields, "observed", "batch"),
                Err(SinaError::Protocol(message)) if message.contains(expected)
            ));
        }
    }
}
