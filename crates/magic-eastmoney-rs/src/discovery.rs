use crate::datacenter_api::fetch_all_rows;
use crate::mapping::{iso_date, optional_f64, optional_string, percent, required_string};
use crate::{source_instrument, BatchContext, EastmoneyClient, EastmoneyError};
use magic_market_core::{
    AssetClass, DragonTigerDiscovery, DragonTigerDiscoveryRequest, DragonTigerEntry, Exchange,
    InstrumentId, Money, NonEmptyText,
};
use serde_json::Value;
use std::collections::HashSet;

impl DragonTigerDiscovery for EastmoneyClient {
    type Error = EastmoneyError;

    fn discover_dragon_tiger(
        &self,
        request: &DragonTigerDiscoveryRequest,
    ) -> Result<magic_market_core::DataBatch<DragonTigerEntry>, Self::Error> {
        let date = request.trading_date().as_str();
        let filter = format!("(TRADE_DATE='{date}')");
        let rows = fetch_all_rows(
            self,
            "RPT_DAILYBILLBOARD_DETAILSNEW",
            &filter,
            "TRADE_ID",
            10_000,
        )?;
        let context = BatchContext::new("dragon-tiger-discovery", Some(date))?;
        let mut source_ids = HashSet::with_capacity(rows.len());
        let mut records = Vec::with_capacity(rows.len().min(request.limit().get() as usize));

        for row in &rows {
            let trade_id = positive_trade_id(row.get("TRADE_ID"))?;
            if !source_ids.insert(trade_id) {
                return Err(EastmoneyError::Protocol(format!(
                    "duplicate Eastmoney dragon-tiger TRADE_ID {trade_id}"
                )));
            }
            let instrument = row_instrument(row)?;
            let source_date_text = required_string(row, "TRADE_DATE")?;
            let source_date = iso_date(&source_date_text)?;
            if &source_date != request.trading_date() {
                return Err(EastmoneyError::Protocol(format!(
                    "Eastmoney dragon-tiger source date {} does not match requested date {date}",
                    source_date.as_str()
                )));
            }

            if request
                .exchange()
                .is_some_and(|exchange| instrument.exchange() != exchange)
            {
                continue;
            }
            if records.len() == request.limit().get() as usize {
                continue;
            }

            records.push(DragonTigerEntry::new(
                NonEmptyText::new(format!("eastmoney:{date}:{trade_id}"))?,
                instrument,
                source_date,
                optional_string(row.get("EXPLANATION"))?
                    .map(NonEmptyText::new)
                    .transpose()?,
                optional_money(row, "BILLBOARD_BUY_AMT")?,
                optional_money(row, "BILLBOARD_SELL_AMT")?,
                optional_money(row, "BILLBOARD_NET_AMT")?,
                percent(optional_f64(row.get("TURNOVERRATE"))?)?,
                context.evidence_at(Some(&source_date_text))?,
            )?);
        }

        context.finish_allow_empty(records)
    }
}

fn positive_trade_id(value: Option<&Value>) -> Result<u64, EastmoneyError> {
    let parsed = match value {
        Some(Value::Number(number)) => number.as_u64(),
        Some(Value::String(text)) => {
            let trimmed = text.trim();
            if trimmed.is_empty() || !trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
                None
            } else {
                trimmed.parse::<u64>().ok()
            }
        }
        _ => None,
    }
    .filter(|value| *value > 0);
    parsed.ok_or_else(|| {
        EastmoneyError::Protocol(
            "dragon-tiger TRADE_ID must be a positive integral source identity".into(),
        )
    })
}

fn row_instrument(row: &Value) -> Result<magic_market_core::InstrumentId, EastmoneyError> {
    let source_code = required_string(row, "SECURITY_CODE")?;
    let secucode = required_string(row, "SECUCODE")?;
    let (secucode_code, suffix) = secucode.split_once('.').ok_or_else(|| {
        EastmoneyError::Protocol(format!(
            "Eastmoney source SECUCODE {secucode:?} has no exchange suffix"
        ))
    })?;
    if secucode_code != source_code {
        return Err(EastmoneyError::Protocol(format!(
            "Eastmoney SECURITY_CODE {source_code:?} disagrees with SECUCODE {secucode:?}"
        )));
    }
    let exchange = match suffix.to_ascii_uppercase().as_str() {
        "SH" => Exchange::Shanghai,
        "SZ" => Exchange::Shenzhen,
        "BJ" => Exchange::Beijing,
        _ => {
            return Err(EastmoneyError::Protocol(format!(
                "unsupported Eastmoney SECUCODE suffix {suffix:?}"
            )))
        }
    };
    let security_type = required_string(row, "SECURITY_TYPE_CODE")?;
    match security_type.as_str() {
        "058001001" => source_instrument(&source_code, exchange),
        "060" => Ok(InstrumentId::new(exchange, source_code, AssetClass::Bond)?),
        _ => Err(EastmoneyError::Protocol(format!(
            "unsupported Eastmoney dragon-tiger SECURITY_TYPE_CODE {security_type:?}"
        ))),
    }
}

fn optional_money(row: &Value, key: &'static str) -> Result<Option<Money>, EastmoneyError> {
    crate::mapping::money(optional_f64(row.get(key))?)
}

#[cfg(test)]
mod tests {
    use super::positive_trade_id;
    use crate::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
    use magic_market_core::{
        AssetClass, DragonTigerDiscovery, DragonTigerDiscoveryRequest, Exchange, IsoDate,
        PositiveU32,
    };
    use serde_json::{json, Value};
    use std::collections::HashSet;

    #[derive(Clone)]
    struct DiscoveryTransport {
        rows: Vec<Value>,
    }

    impl DiscoveryTransport {
        fn fixture() -> Self {
            Self {
                rows: vec![
                    row(101, "600000", "600000.SH", 100.0, 40.0, 60.0),
                    row(102, "123275", "123275.SZ", 80.0, 30.0, 50.0),
                    row(103, "920001", "920001.BJ", 70.0, 20.0, 50.0),
                    row(104, "600000", "600000.SH", 20.0, 10.0, 10.0),
                ],
            }
        }
    }

    impl EastmoneyTransport for DiscoveryTransport {
        fn get(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _max_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            serde_json::to_vec(&json!({
                "success": true,
                "code": 0,
                "result": {
                    "data": self.rows,
                    "pages": 1,
                    "count": self.rows.len()
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
                "discovery fixture does not accept POST".into(),
            ))
        }
    }

    fn row(trade_id: u64, code: &str, secucode: &str, buy: f64, sell: f64, net: f64) -> Value {
        json!({
            "TRADE_ID": trade_id,
            "SECURITY_CODE": code,
            "SECUCODE": secucode,
            "SECURITY_TYPE_CODE": if code.starts_with('1') { "060" } else { "058001001" },
            "TRADE_DATE": "2026-07-24 00:00:00",
            "EXPLANATION": "fixture reason",
            "BILLBOARD_BUY_AMT": buy,
            "BILLBOARD_SELL_AMT": sell,
            "BILLBOARD_NET_AMT": net,
            "TURNOVERRATE": 1.5
        })
    }

    fn request(limit: u32) -> DragonTigerDiscoveryRequest {
        DragonTigerDiscoveryRequest::new(
            IsoDate::new("2026-07-24").unwrap(),
            PositiveU32::new(limit).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn discovers_all_exchanges_and_keeps_multi_reason_ids_unique() {
        let client = EastmoneyClient::with_transport(DiscoveryTransport::fixture());
        let batch = client.discover_dragon_tiger(&request(10_000)).unwrap();
        assert_eq!(batch.records().len(), 4);
        assert_eq!(
            batch
                .records()
                .iter()
                .map(|row| row.entry_id().as_str())
                .collect::<HashSet<_>>()
                .len(),
            4
        );
        assert!(batch
            .records()
            .iter()
            .any(|row| row.instrument().exchange() == Exchange::Beijing));
        assert!(batch
            .records()
            .iter()
            .any(|row| row.instrument().asset_class() == AssetClass::Bond));
        assert_eq!(
            batch.records()[0].entry_id().as_str(),
            "eastmoney:2026-07-24:101"
        );
    }

    #[test]
    fn reads_complete_day_before_exchange_filter_and_limit() {
        let client = EastmoneyClient::with_transport(DiscoveryTransport::fixture());
        let filtered = request(1).with_exchange(Exchange::Shanghai);
        let batch = client.discover_dragon_tiger(&filtered).unwrap();
        assert_eq!(batch.records().len(), 1);
        assert_eq!(
            batch.records()[0].instrument().exchange(),
            Exchange::Shanghai
        );
    }

    #[test]
    fn rejects_duplicate_or_non_integral_trade_ids() {
        let mut duplicate = DiscoveryTransport::fixture();
        duplicate.rows[1]["TRADE_ID"] = json!(101);
        assert!(EastmoneyClient::with_transport(duplicate)
            .discover_dragon_tiger(&request(10))
            .is_err());
        for invalid in [json!(0), json!(1.5), json!("1.0"), json!("not-an-id")] {
            assert!(positive_trade_id(Some(&invalid)).is_err());
        }
    }

    #[test]
    fn rejects_identity_date_and_financial_invariant_failures() {
        let mut disagreement = DiscoveryTransport::fixture();
        disagreement.rows[0]["SECUCODE"] = json!("600001.SH");
        assert!(EastmoneyClient::with_transport(disagreement)
            .discover_dragon_tiger(&request(10))
            .is_err());

        let mut wrong_date = DiscoveryTransport::fixture();
        wrong_date.rows[0]["TRADE_DATE"] = json!("2026-07-23 00:00:00");
        assert!(EastmoneyClient::with_transport(wrong_date)
            .discover_dragon_tiger(&request(10))
            .is_err());

        let mut wrong_net = DiscoveryTransport::fixture();
        wrong_net.rows[0]["BILLBOARD_NET_AMT"] = json!(59.0);
        assert!(EastmoneyClient::with_transport(wrong_net)
            .discover_dragon_tiger(&request(10))
            .is_err());
    }
}
