use crate::datacenter_api::fetch_rows;
use crate::mapping::{iso_date, money, optional_f64, optional_string, percent, required_string};
use crate::{
    validate_instrument, validate_source_instrument, validate_source_secucode, BatchContext,
    EastmoneyClient, EastmoneyError,
};
use magic_market_core::{
    DragonTigerData, DragonTigerEntry, DragonTigerSeat, DragonTigerSide, InstrumentSignalRequest,
    Money, NonEmptyText, PositiveU32,
};
use serde_json::Value;

const SEAT_SIDE_CARDINALITY: u32 = 5;
const SEAT_SIDE_FETCH_LIMIT: u32 = SEAT_SIDE_CARDINALITY + 1;

impl DragonTigerData for EastmoneyClient {
    type Error = EastmoneyError;

    fn dragon_tiger_entries(
        &self,
        request: &InstrumentSignalRequest,
    ) -> Result<magic_market_core::DataBatch<DragonTigerEntry>, Self::Error> {
        validate_instrument(request.instrument())?;
        let filter = signal_filter(request, None);
        let rows = fetch_rows(
            self,
            "RPT_DAILYBILLBOARD_DETAILSNEW",
            &filter,
            "TRADE_DATE",
            request.limit().get(),
        )?;
        map_entries(&rows, request)
    }

    fn dragon_tiger_seats(
        &self,
        request: &InstrumentSignalRequest,
    ) -> Result<magic_market_core::DataBatch<DragonTigerSeat>, Self::Error> {
        validate_instrument(request.instrument())?;
        validate_seat_limit(request)?;
        let source_date = match request.trading_date() {
            Some(date) => Some(date.as_str().to_owned()),
            None => {
                let latest = fetch_rows(
                    self,
                    "RPT_DAILYBILLBOARD_DETAILSNEW",
                    &signal_filter(request, None),
                    "TRADE_DATE",
                    1,
                )?;
                if let Some(row) = latest.first() {
                    validate_signal_row(row, request, None)?;
                }
                latest
                    .first()
                    .map(|row| required_string(row, "TRADE_DATE"))
                    .transpose()?
                    .map(|value| {
                        value
                            .get(..10)
                            .ok_or_else(|| {
                                EastmoneyError::Protocol(
                                    "dragon-tiger date has no YYYY-MM-DD prefix".into(),
                                )
                            })
                            .map(str::to_owned)
                    })
                    .transpose()?
            }
        };
        let Some(source_date) = source_date else {
            return BatchContext::new("dragon-tiger-seats", None)?.finish(Vec::new());
        };
        let filter = signal_filter(request, Some(&source_date));
        let mut rows = Vec::new();
        for (report, side, sort) in [
            ("RPT_BILLBOARD_DAILYDETAILSBUY", DragonTigerSide::Buy, "BUY"),
            (
                "RPT_BILLBOARD_DAILYDETAILSSELL",
                DragonTigerSide::Sell,
                "SELL",
            ),
        ] {
            // Read one sentinel row beyond the admitted top five so an
            // oversized upstream result cannot be hidden by fetch_rows'
            // caller-limit truncation.
            let side_rows = fetch_rows(self, report, &filter, sort, SEAT_SIDE_FETCH_LIMIT)?;
            rows.extend(side_rows.into_iter().map(|row| (side, row)));
        }
        map_seats(&rows, request, &source_date)
    }
}

fn validate_seat_limit(request: &InstrumentSignalRequest) -> Result<(), EastmoneyError> {
    if request.limit().get() < 10 {
        return Err(EastmoneyError::InvalidRequest(
            "dragon-tiger seat limit must be at least 10 for one complete buy-five/sell-five group"
                .into(),
        ));
    }
    Ok(())
}

fn signal_filter(request: &InstrumentSignalRequest, forced_date: Option<&str>) -> String {
    let mut filter = format!("(SECURITY_CODE=\"{}\")", request.instrument().code());
    let date = forced_date.or_else(|| {
        request
            .trading_date()
            .map(magic_market_core::IsoDate::as_str)
    });
    if let Some(date) = date {
        filter.push_str(&format!("(TRADE_DATE='{date}')"));
    }
    filter
}

fn map_entries(
    rows: &[Value],
    request: &InstrumentSignalRequest,
) -> Result<magic_market_core::DataBatch<DragonTigerEntry>, EastmoneyError> {
    let source_at = rows
        .iter()
        .filter_map(|row| optional_string(row.get("TRADE_DATE")).ok().flatten())
        .max();
    let context = BatchContext::new("dragon-tiger-entries", source_at.as_deref())?;
    let records = rows
        .iter()
        .map(|row| {
            validate_signal_row(
                row,
                request,
                request.trading_date().map(|date| date.as_str()),
            )?;
            let date = required_string(row, "TRADE_DATE")?;
            Ok(DragonTigerEntry::new(
                entry_id(request.instrument().code(), &date)?,
                request.instrument().clone(),
                iso_date(&date)?,
                optional_string(row.get("EXPLANATION"))?
                    .map(NonEmptyText::new)
                    .transpose()?,
                opt_money(row, "BILLBOARD_BUY_AMT")?,
                opt_money(row, "BILLBOARD_SELL_AMT")?,
                opt_money(row, "BILLBOARD_NET_AMT")?,
                percent(optional_f64(row.get("TURNOVERRATE"))?)?,
                context.evidence_at(Some(&date))?,
            )?)
        })
        .collect::<Result<Vec<_>, EastmoneyError>>()?;
    context.finish(records)
}

fn map_seats(
    rows: &[(DragonTigerSide, Value)],
    request: &InstrumentSignalRequest,
    source_date: &str,
) -> Result<magic_market_core::DataBatch<DragonTigerSeat>, EastmoneyError> {
    let buy_count = rows
        .iter()
        .filter(|(side, _)| *side == DragonTigerSide::Buy)
        .count();
    let sell_count = rows
        .iter()
        .filter(|(side, _)| *side == DragonTigerSide::Sell)
        .count();
    if rows.len() != 10
        || buy_count != SEAT_SIDE_CARDINALITY as usize
        || sell_count != SEAT_SIDE_CARDINALITY as usize
    {
        return Err(EastmoneyError::Protocol(format!(
            "dragon-tiger seats require exactly five buy and five sell rows; got {buy_count} buy and {sell_count} sell"
        )));
    }
    let context = BatchContext::new("dragon-tiger-seats", Some(source_date))?;
    let mut buy_rank = 0_u32;
    let mut sell_rank = 0_u32;
    let records = rows
        .iter()
        .map(|(side, row)| {
            validate_signal_row(row, request, Some(source_date))?;
            let rank = match side {
                DragonTigerSide::Buy => {
                    buy_rank = buy_rank
                        .checked_add(1)
                        .ok_or_else(|| EastmoneyError::Protocol("buy rank overflow".into()))?;
                    buy_rank
                }
                DragonTigerSide::Sell => {
                    sell_rank = sell_rank
                        .checked_add(1)
                        .ok_or_else(|| EastmoneyError::Protocol("sell rank overflow".into()))?;
                    sell_rank
                }
            };
            let amount_key = match side {
                DragonTigerSide::Buy => "BUY",
                DragonTigerSide::Sell => "SELL",
            };
            let amount = required_money(row, amount_key)?;
            Ok(DragonTigerSeat::new(
                entry_id(request.instrument().code(), source_date)?,
                request.instrument().clone(),
                iso_date(source_date)?,
                *side,
                PositiveU32::new(rank)?,
                NonEmptyText::new(required_string(row, "OPERATEDEPT_NAME")?)?,
                amount,
                opt_money(row, "BUY")?,
                opt_money(row, "SELL")?,
                opt_money(row, "NET")?,
                context.evidence_at(Some(source_date))?,
            )?)
        })
        .collect::<Result<Vec<_>, EastmoneyError>>()?;
    context.finish(records)
}

fn entry_id(code: &str, date: &str) -> Result<NonEmptyText, EastmoneyError> {
    let date = date
        .get(..10)
        .ok_or_else(|| EastmoneyError::Protocol("dragon-tiger date is too short".into()))?;
    Ok(NonEmptyText::new(format!("{code}:{date}"))?)
}

fn validate_signal_row(
    row: &Value,
    request: &InstrumentSignalRequest,
    expected_date: Option<&str>,
) -> Result<(), EastmoneyError> {
    // All three verified dragon-tiger datacenter reports use the same real
    // identity fields: SECURITY_CODE + SECUCODE. Require both so the request
    // filter cannot be mistaken for response-side provenance.
    let source_code = required_string(row, "SECURITY_CODE")?;
    let secucode = required_string(row, "SECUCODE")?;
    validate_source_instrument(request.instrument(), &source_code, None)?;
    validate_source_secucode(request.instrument(), &secucode)?;
    let source_date = required_string(row, "TRADE_DATE")?;
    let actual = iso_date(&source_date)?;
    if let Some(expected_date) = expected_date {
        let expected = iso_date(expected_date)?;
        if actual != expected {
            return Err(EastmoneyError::Protocol(format!(
                "Eastmoney dragon-tiger source date {} does not match requested date {}",
                actual.as_str(),
                expected.as_str()
            )));
        }
    }
    Ok(())
}

fn opt_money(row: &Value, key: &'static str) -> Result<Option<Money>, EastmoneyError> {
    money(optional_f64(row.get(key))?)
}

fn required_money(row: &Value, key: &'static str) -> Result<Money, EastmoneyError> {
    opt_money(row, key)?
        .ok_or_else(|| EastmoneyError::Protocol(format!("dragon-tiger field {key} is absent")))
}

#[cfg(test)]
mod tests {
    use super::{
        map_entries, map_seats, validate_seat_limit, SEAT_SIDE_CARDINALITY, SEAT_SIDE_FETCH_LIMIT,
    };
    use crate::{EastmoneyClient, EastmoneyError, EastmoneyTransport};
    use magic_market_core::{
        AssetClass, DragonTigerData, DragonTigerSide, Exchange, InstrumentId,
        InstrumentSignalRequest, IsoDate, PositiveU32, RatioUnit,
    };
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    struct SeatTransport {
        buy_rows: usize,
        sell_rows: usize,
        requests: Arc<Mutex<Vec<String>>>,
    }

    impl EastmoneyTransport for SeatTransport {
        fn get(
            &self,
            url: &str,
            _headers: &[(&str, &str)],
            _max_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            self.requests.lock().unwrap().push(url.to_owned());
            let (side, count) = if url.contains("reportName=RPT_BILLBOARD_DAILYDETAILSBUY") {
                (DragonTigerSide::Buy, self.buy_rows)
            } else if url.contains("reportName=RPT_BILLBOARD_DAILYDETAILSSELL") {
                (DragonTigerSide::Sell, self.sell_rows)
            } else {
                return Err(EastmoneyError::InvalidRequest(
                    "seat fixture received an unexpected report".into(),
                ));
            };
            let rows = (1..=count)
                .map(|rank| scripted_seat_row(side, rank))
                .collect::<Vec<_>>();
            serde_json::to_vec(&json!({
                "success": true,
                "code": 0,
                "result": {"data": rows, "pages": 1}
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
                "seat fixture does not accept POST".into(),
            ))
        }
    }

    fn scripted_seat_row(side: DragonTigerSide, rank: usize) -> serde_json::Value {
        let (buy, sell, name) = match side {
            DragonTigerSide::Buy => (100 + rank, 10, format!("买方机构{rank}")),
            DragonTigerSide::Sell => (5, 80 + rank, format!("卖方机构{rank}")),
        };
        json!({
            "SECURITY_CODE":"002475",
            "SECUCODE":"002475.SZ",
            "TRADE_DATE":"2026-07-23 00:00:00",
            "OPERATEDEPT_NAME":name,
            "BUY":buy,
            "SELL":sell,
            "NET":buy as isize - sell as isize
        })
    }

    fn request() -> InstrumentSignalRequest {
        InstrumentSignalRequest::new(
            InstrumentId::new(Exchange::Shenzhen, "002475", AssetClass::Equity).unwrap(),
            PositiveU32::new(10).unwrap(),
        )
        .unwrap()
    }

    fn dated_request() -> InstrumentSignalRequest {
        request().with_trading_date(IsoDate::new("2026-07-23").unwrap())
    }

    fn complete_seat_rows() -> Vec<(DragonTigerSide, serde_json::Value)> {
        let mut rows = Vec::new();
        for rank in 1..=5 {
            let buy = 100 + rank;
            rows.push((
                DragonTigerSide::Buy,
                json!({
                    "SECURITY_CODE":"002475",
                    "SECUCODE":"002475.SZ",
                    "TRADE_DATE":"2026-07-23 00:00:00",
                    "OPERATEDEPT_NAME":format!("买方机构{rank}"),
                    "BUY":buy,
                    "SELL":10,
                    "NET":buy - 10
                }),
            ));
        }
        for rank in 1..=5 {
            let sell = 80 + rank;
            rows.push((
                DragonTigerSide::Sell,
                json!({
                    "SECURITY_CODE":"002475",
                    "SECUCODE":"002475.SZ",
                    "TRADE_DATE":"2026-07-23 00:00:00",
                    "OPERATEDEPT_NAME":format!("卖方机构{rank}"),
                    "BUY":5,
                    "SELL":sell,
                    "NET":5 - sell
                }),
            ));
        }
        rows
    }

    #[test]
    fn seat_request_limit_reserves_one_atomic_top_five_group() {
        let instrument =
            InstrumentId::new(Exchange::Shenzhen, "002475", AssetClass::Equity).unwrap();
        let too_small =
            InstrumentSignalRequest::new(instrument.clone(), PositiveU32::new(9).unwrap()).unwrap();
        let exact = InstrumentSignalRequest::new(instrument.clone(), PositiveU32::new(10).unwrap())
            .unwrap();
        let larger =
            InstrumentSignalRequest::new(instrument, PositiveU32::new(100).unwrap()).unwrap();
        assert!(validate_seat_limit(&too_small).is_err());
        assert!(validate_seat_limit(&exact).is_ok());
        assert!(validate_seat_limit(&larger).is_ok());
        assert_eq!(SEAT_SIDE_CARDINALITY, 5);
        assert_eq!(SEAT_SIDE_FETCH_LIMIT, 6);
    }

    #[test]
    fn maps_entry_amounts_reason_turnover_and_evidence() {
        let batch = map_entries(
            &[json!({"SECURITY_CODE":"002475","SECUCODE":"002475.SZ",
                "TRADE_DATE":"2026-07-23 00:00:00",
                "EXPLANATION":"日涨幅偏离值达到7%",
                "BILLBOARD_BUY_AMT":100,"BILLBOARD_SELL_AMT":40,
                "BILLBOARD_NET_AMT":60,"TURNOVERRATE":12.5})],
            &request(),
        )
        .unwrap();
        let entry = &batch.records()[0];
        assert_eq!(entry.entry_id().as_str(), "002475:2026-07-23");
        assert_eq!(entry.trading_date().as_str(), "2026-07-23");
        assert_eq!(entry.reason().unwrap().as_str(), "日涨幅偏离值达到7%");
        assert_eq!(entry.buy_amount().unwrap().get(), 100.0);
        assert_eq!(entry.sell_amount().unwrap().get(), 40.0);
        assert_eq!(entry.net_amount().unwrap().get(), 60.0);
        assert_eq!(entry.turnover_rate().unwrap().get(), 12.5);
        assert_eq!(entry.turnover_rate().unwrap().unit(), RatioUnit::Percent);
        assert_eq!(entry.evidence().source_at(), Some("2026-07-23 00:00:00"));
    }

    #[test]
    fn maps_buy_and_sell_seats_with_independent_ranks() {
        let rows = complete_seat_rows();
        let batch = map_seats(&rows, &request(), "2026-07-23").unwrap();
        assert_eq!(batch.records().len(), 10);
        assert_eq!(batch.records()[0].side(), DragonTigerSide::Buy);
        assert_eq!(batch.records()[0].rank().get(), 1);
        assert_eq!(batch.records()[0].seat_name().as_str(), "买方机构1");
        assert_eq!(batch.records()[0].amount().get(), 101.0);
        assert_eq!(batch.records()[0].buy_amount().unwrap().get(), 101.0);
        assert_eq!(batch.records()[0].sell_amount().unwrap().get(), 10.0);
        assert_eq!(batch.records()[0].net_amount().unwrap().get(), 91.0);
        assert_eq!(batch.records()[5].side(), DragonTigerSide::Sell);
        assert_eq!(batch.records()[5].rank().get(), 1);
        assert_eq!(batch.records()[5].amount().get(), 81.0);
    }

    #[test]
    fn rejects_incomplete_or_oversized_seat_groups() {
        let mut incomplete = complete_seat_rows();
        incomplete.pop();
        assert!(map_seats(&incomplete, &request(), "2026-07-23").is_err());

        let mut oversized = complete_seat_rows();
        oversized.push(oversized[0].clone());
        assert!(map_seats(&oversized, &request(), "2026-07-23").is_err());
    }

    #[test]
    fn trait_path_uses_the_sixth_row_sentinel_before_local_truncation() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let client = EastmoneyClient::with_transport(SeatTransport {
            buy_rows: 5,
            sell_rows: 5,
            requests: Arc::clone(&requests),
        });
        let batch = client.dragon_tiger_seats(&dated_request()).unwrap();
        assert_eq!(batch.records().len(), 10);
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests
            .iter()
            .any(|url| url.contains("reportName=RPT_BILLBOARD_DAILYDETAILSBUY")));
        assert!(requests
            .iter()
            .any(|url| url.contains("reportName=RPT_BILLBOARD_DAILYDETAILSSELL")));
        drop(requests);

        for (buy_rows, sell_rows, expected) in [
            (6, 5, "got 6 buy and 5 sell"),
            (5, 6, "got 5 buy and 6 sell"),
        ] {
            let client = EastmoneyClient::with_transport(SeatTransport {
                buy_rows,
                sell_rows,
                requests: Arc::new(Mutex::new(Vec::new())),
            });
            let error = client.dragon_tiger_seats(&dated_request()).unwrap_err();
            assert!(
                matches!(error, EastmoneyError::Protocol(message) if message.contains(expected))
            );
        }
    }

    #[test]
    fn source_code_and_requested_trading_date_must_match() {
        let request =
            request().with_trading_date(magic_market_core::IsoDate::new("2026-07-23").unwrap());
        let wrong_code = map_entries(
            &[json!({
                "SECURITY_CODE":"600396",
                "SECUCODE":"600396.SH",
                "TRADE_DATE":"2026-07-23",
                "BILLBOARD_BUY_AMT":1
            })],
            &request,
        );
        assert!(matches!(
            wrong_code,
            Err(crate::EastmoneyError::Protocol(_))
        ));
        let wrong_date = map_entries(
            &[json!({
                "SECURITY_CODE":"002475",
                "SECUCODE":"002475.SZ",
                "TRADE_DATE":"2026-07-22",
                "BILLBOARD_BUY_AMT":1
            })],
            &request,
        );
        assert!(matches!(
            wrong_date,
            Err(crate::EastmoneyError::Protocol(_))
        ));
        let wrong_seat = map_seats(
            &[(
                DragonTigerSide::Buy,
                json!({
                    "SECURITY_CODE":"600396",
                    "SECUCODE":"600396.SH",
                    "TRADE_DATE":"2026-07-23",
                    "OPERATEDEPT_NAME":"x",
                    "BUY":1
                }),
            )],
            &request,
            "2026-07-23",
        );
        assert!(matches!(
            wrong_seat,
            Err(crate::EastmoneyError::Protocol(_))
        ));
    }

    #[test]
    fn every_entry_and_seat_requires_the_real_identity_pair() {
        for row in [
            json!({
                "SECURITY_CODE":"002475",
                "TRADE_DATE":"2026-07-23",
                "BILLBOARD_BUY_AMT":1
            }),
            json!({
                "SECUCODE":"002475.SZ",
                "TRADE_DATE":"2026-07-23",
                "BILLBOARD_BUY_AMT":1
            }),
        ] {
            assert!(matches!(
                map_entries(&[row], &request()),
                Err(crate::EastmoneyError::Protocol(_))
            ));
        }
        for row in [
            json!({
                "SECURITY_CODE":"002475",
                "TRADE_DATE":"2026-07-23",
                "OPERATEDEPT_NAME":"x",
                "BUY":1
            }),
            json!({
                "SECUCODE":"002475.SZ",
                "TRADE_DATE":"2026-07-23",
                "OPERATEDEPT_NAME":"x",
                "BUY":1
            }),
        ] {
            assert!(matches!(
                map_seats(&[(DragonTigerSide::Buy, row)], &request(), "2026-07-23"),
                Err(crate::EastmoneyError::Protocol(_))
            ));
        }
    }

    #[test]
    fn every_seat_requires_a_matching_source_trade_date() {
        for row in [
            json!({
                "SECURITY_CODE":"002475",
                "SECUCODE":"002475.SZ",
                "OPERATEDEPT_NAME":"x",
                "BUY":1
            }),
            json!({
                "SECURITY_CODE":"002475",
                "SECUCODE":"002475.SZ",
                "TRADE_DATE":"2026-07-22",
                "OPERATEDEPT_NAME":"x",
                "BUY":1
            }),
        ] {
            assert!(matches!(
                map_seats(&[(DragonTigerSide::Buy, row)], &request(), "2026-07-23"),
                Err(crate::EastmoneyError::Protocol(_))
            ));
        }
    }
}
