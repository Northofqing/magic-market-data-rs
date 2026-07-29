use crate::{now, validate_instruments, TencentClient, TencentError};
use magic_market_core::{
    DataBatch, DataStatus, NumericTolerance, Price, ProviderId, Quantity, Trade, TradeSide, Trades,
    TradesRequest,
};
use serde_json::Value;

const TRADES_ENDPOINT: &str = "https://stock.gtimg.cn/data/index.php?appn=detail&action=data&c=";
const PAGE_SIZE: usize = 70;
const MAX_TRADES: u16 = 2_000;

#[derive(Debug)]
pub(crate) struct SourceTrade {
    sequence: u32,
    time: String,
    price: f64,
    quantity_lots: f64,
    amount_yuan: f64,
    side: TradeSide,
}

fn number(value: &str, field: &'static str) -> Result<f64, TencentError> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| TencentError::Protocol(format!("{field} is not numeric: {value:?}")))?;
    if !parsed.is_finite() {
        return Err(TencentError::Protocol(format!("{field} must be finite")));
    }
    Ok(parsed)
}

fn valid_trade_time(value: &str) -> bool {
    if value.len() != 8 || value.as_bytes()[2] != b':' || value.as_bytes()[5] != b':' {
        return false;
    }
    let hour = value[0..2].parse::<u8>().unwrap_or(24);
    let minute = value[3..5].parse::<u8>().unwrap_or(60);
    let second = value[6..8].parse::<u8>().unwrap_or(60);
    hour < 24 && minute < 60 && second < 60
}

pub(crate) fn parse_trade_page(
    bytes: &[u8],
    symbol: &str,
    expected_page: u16,
) -> Result<Vec<SourceTrade>, TencentError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| TencentError::Decode(format!("trade response UTF-8: {error}")))?;
    let prefix = format!("v_detail_data_{symbol}=");
    let payload = text
        .trim()
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix(';').or(Some(value)))
        .ok_or_else(|| TencentError::Protocol("trade wrapper symbol mismatch".into()))?;
    let value: Value = serde_json::from_str(payload)
        .map_err(|error| TencentError::Decode(format!("trade wrapper JSON: {error}")))?;
    let wrapper = value
        .as_array()
        .filter(|values| values.len() == 2)
        .ok_or_else(|| TencentError::Protocol("trade wrapper must have two fields".into()))?;
    let page = wrapper[0]
        .as_u64()
        .ok_or_else(|| TencentError::Protocol("trade page number is missing".into()))?;
    if page != u64::from(expected_page) {
        return Err(TencentError::Protocol(format!(
            "trade page mismatch: requested {expected_page}, received {page}"
        )));
    }
    let encoded = wrapper[1]
        .as_str()
        .ok_or_else(|| TencentError::Protocol("trade rows must be a string".into()))?;
    if encoded.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<_> = encoded.split('|').collect();
    if rows.len() > PAGE_SIZE {
        return Err(TencentError::Protocol(format!(
            "trade page has {} records; maximum is {PAGE_SIZE}",
            rows.len()
        )));
    }
    let mut parsed = Vec::with_capacity(rows.len());
    for row in rows {
        let fields: Vec<_> = row.split('/').collect();
        if fields.len() != 7 {
            return Err(TencentError::Protocol(format!(
                "trade row has {} fields; expected 7",
                fields.len()
            )));
        }
        let sequence = fields[0]
            .parse::<u32>()
            .map_err(|_| TencentError::Protocol("trade sequence is invalid".into()))?;
        if !valid_trade_time(fields[1]) {
            return Err(TencentError::Protocol(format!(
                "trade time is invalid: {:?}",
                fields[1]
            )));
        }
        let price = number(fields[2], "trade price")?;
        let _change = number(fields[3], "trade change")?;
        let quantity_lots = number(fields[4], "trade quantity")?;
        let amount_yuan = number(fields[5], "trade amount")?;
        if price <= 0.0 || quantity_lots < 0.0 || amount_yuan < 0.0 {
            return Err(TencentError::Protocol(
                "trade price/quantity/amount is outside valid bounds".into(),
            ));
        }
        let expected_amount = price * quantity_lots * 100.0;
        // Preserve Tencent's source contract: two percent of the reference
        // amount plus CNY 100, rather than scaling against the larger operand.
        let tolerance = NumericTolerance::new(expected_amount.abs().mul_add(0.02, 100.0), 0.0)?;
        if !tolerance.matches(amount_yuan, expected_amount) {
            return Err(TencentError::Protocol(format!(
                "trade amount contradicts price and source-lot quantity at sequence {sequence}"
            )));
        }
        let side = match fields[6] {
            "B" => TradeSide::Buy,
            "S" => TradeSide::Sell,
            "M" => TradeSide::Neutral,
            value => {
                return Err(TencentError::Protocol(format!(
                    "unknown Tencent trade side {value:?}"
                )));
            }
        };
        parsed.push(SourceTrade {
            sequence,
            time: fields[1].to_owned(),
            price,
            quantity_lots,
            amount_yuan,
            side,
        });
    }
    Ok(parsed)
}

impl Trades for TencentClient {
    type Error = TencentError;

    fn trades(&self, request: &TradesRequest) -> Result<DataBatch<Trade>, Self::Error> {
        if request.date().is_some() {
            return Err(TencentError::Unsupported(
                "Tencent detail endpoint has no verified historical date selector".into(),
            ));
        }
        if request.limit() > MAX_TRADES {
            return Err(TencentError::InvalidRequest(format!(
                "Tencent trades accept at most {MAX_TRADES} records"
            )));
        }
        if request.instrument().exchange() == magic_market_core::Exchange::Beijing {
            return Err(TencentError::Unsupported(
                "Tencent detail endpoint returned no Beijing trade records in live validation"
                    .into(),
            ));
        }
        let symbol = validate_instruments(std::slice::from_ref(request.instrument()))?
            .pop()
            .ok_or_else(|| TencentError::InvalidRequest("trade instrument is missing".into()))?;
        let mut source = Vec::with_capacity(usize::from(request.limit()));
        let mut page = 0_u16;
        let mut expected_sequence: Option<u32> = None;
        while source.len() < usize::from(request.limit()) {
            let url = format!("{TRADES_ENDPOINT}{symbol}&p={page}");
            let records = parse_trade_page(&self.transport.get(&url)?, &symbol, page)?;
            if records.is_empty() {
                break;
            }
            for record in records
                .iter()
                .take(usize::from(request.limit()) - source.len())
            {
                if expected_sequence.is_some_and(|expected| record.sequence != expected) {
                    return Err(TencentError::Protocol(format!(
                        "trade sequence gap: expected {}, received {}",
                        expected_sequence.unwrap_or_default(),
                        record.sequence
                    )));
                }
                expected_sequence = record.sequence.checked_add(1);
                source.push(SourceTrade {
                    sequence: record.sequence,
                    time: record.time.clone(),
                    price: record.price,
                    quantity_lots: record.quantity_lots,
                    amount_yuan: record.amount_yuan,
                    side: record.side,
                });
            }
            if records.len() < PAGE_SIZE {
                break;
            }
            page = page
                .checked_add(1)
                .ok_or_else(|| TencentError::Protocol("trade page overflow".into()))?;
        }
        if source.is_empty() {
            return Err(TencentError::Protocol("trade response is empty".into()));
        }
        let observed_at = now()?;
        let batch_id = format!("tencent-web:{observed_at}:trades");
        let mut records = Vec::with_capacity(source.len());
        for item in source {
            records.push(Trade::new(
                request.instrument().clone(),
                item.time.clone(),
                Price::new(item.price)?,
                Quantity::new(item.quantity_lots)?,
                None,
                item.side,
                DataStatus::Available,
                Some(item.time),
                observed_at.clone(),
                ProviderId::Tencent,
                batch_id.clone(),
            )?);
        }
        let latest_source_at = records
            .last()
            .and_then(Trade::source_at)
            .ok_or_else(|| TencentError::Protocol("trade source time is missing".into()))?;
        let provenance = magic_market_core::Provenance::new("tencent-web", observed_at)?
            .with_source_at(latest_source_at)?
            .with_batch_id(batch_id)?;
        Ok(DataBatch::strict(records, provenance))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_verified_trade_fields_and_sides() {
        let fixture = br#"v_detail_data_sh600396=[0,"0/09:25:01/15.30/0.23/66238/101344140/B|1/09:30:01/15.37/0.07/48629/74642375/S|2/09:30:04/15.39/0.02/8566/13177435/M"]"#;
        let rows = parse_trade_page(fixture, "sh600396", 0).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].sequence, 0);
        assert_eq!(rows[0].side, TradeSide::Buy);
        assert_eq!(rows[1].side, TradeSide::Sell);
        assert_eq!(rows[2].side, TradeSide::Neutral);
        assert_eq!(rows[0].quantity_lots, 66_238.0);
    }

    #[test]
    fn rejects_wrapper_page_side_and_amount_mismatches() {
        let fixture = br#"v_detail_data_sh600396=[1,"0/09:25:01/15.30/0.23/10/15300/B"]"#;
        assert!(parse_trade_page(fixture, "sh600396", 0).is_err());
        let bad_side = br#"v_detail_data_sh600396=[0,"0/09:25:01/15.30/0.23/10/15300/X"]"#;
        assert!(parse_trade_page(bad_side, "sh600396", 0).is_err());
        let bad_amount = br#"v_detail_data_sh600396=[0,"0/09:25:01/15.30/0.23/10/1/B"]"#;
        assert!(parse_trade_page(bad_amount, "sh600396", 0).is_err());
        let exact_tolerance = br#"v_detail_data_sh600396=[0,"0/09:25:01/1.00/0.00/100/10300/B"]"#;
        assert!(parse_trade_page(exact_tolerance, "sh600396", 0).is_ok());
        let above_reference_tolerance =
            br#"v_detail_data_sh600396=[0,"0/09:25:01/1.00/0.00/100/10305/B"]"#;
        assert!(parse_trade_page(above_reference_tolerance, "sh600396", 0).is_err());
        let wrong_symbol = br#"v_detail_data_sz000001=[0,"0/09:25:01/15.30/0.23/10/15300/B"]"#;
        assert!(parse_trade_page(wrong_symbol, "sh600396", 0).is_err());
    }
}
