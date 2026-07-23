use crate::bars::{kline_url, parse_source_rows};
use crate::{now, shares_to_lots, validate_instruments, SinaClient, SinaError};
use magic_market_core::{
    BarInterval, DataBatch, DataStatus, MinuteData, MinuteDataRequest, MinutePoint, Money, Price,
    ProviderId, Quantity,
};

const MAX_MINUTE_BARS: u16 = 300;

pub(crate) fn parse_current_minutes(
    bytes: &[u8],
    instrument: &magic_market_core::InstrumentId,
    observed_at: &str,
) -> Result<DataBatch<MinutePoint>, SinaError> {
    let rows = parse_source_rows(bytes, BarInterval::Minute1, MAX_MINUTE_BARS)?;
    let latest_date = rows
        .iter()
        .map(|row| &row.bar_time[..10])
        .max()
        .ok_or_else(|| SinaError::Protocol("current minute response has no date".into()))?
        .to_owned();
    let batch_id = format!("sina-web:{observed_at}:minute");
    let mut records = Vec::new();
    let mut cumulative_shares = 0.0_f64;
    let mut cumulative_amount = 0.0_f64;
    for row in rows
        .into_iter()
        .filter(|row| row.bar_time.starts_with(&latest_date))
    {
        cumulative_shares += row.volume_shares;
        cumulative_amount += row
            .amount_yuan
            .ok_or_else(|| SinaError::Protocol("minute amount is missing".into()))?;
        if !cumulative_shares.is_finite() || !cumulative_amount.is_finite() {
            return Err(SinaError::Protocol(
                "minute cumulative quantity or amount overflowed".into(),
            ));
        }
        let minute_at = row
            .bar_time
            .get(..16)
            .ok_or_else(|| SinaError::Protocol("minute timestamp is too short".into()))?;
        records.push(MinutePoint::new(
            instrument.clone(),
            minute_at,
            Price::new(row.close)?,
            Quantity::new(shares_to_lots(cumulative_shares)?)?,
            Some(Money::new(cumulative_amount)?),
            DataStatus::Available,
            Some(row.source_at),
            observed_at,
            ProviderId::Sina,
            batch_id.clone(),
        )?);
    }
    let latest_source_at = records
        .last()
        .and_then(MinutePoint::source_at)
        .ok_or_else(|| SinaError::Protocol("current minute response is empty".into()))?;
    let provenance = magic_market_core::Provenance::new("sina-web", observed_at)?
        .with_source_at(latest_source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

impl MinuteData for SinaClient {
    type Error = SinaError;

    fn minute_data(
        &self,
        request: &MinuteDataRequest,
    ) -> Result<DataBatch<MinutePoint>, Self::Error> {
        if request.date().is_some() {
            return Err(SinaError::Unsupported(
                "Sina current-minute derivation has no verified historical date selector".into(),
            ));
        }
        let symbol = validate_instruments(std::slice::from_ref(request.instrument()))?
            .pop()
            .ok_or_else(|| SinaError::InvalidRequest("minute instrument is missing".into()))?;
        let url = kline_url(&symbol, BarInterval::Minute1, MAX_MINUTE_BARS)?;
        let bytes = self.transport.get(&url)?;
        let observed_at = now()?;
        parse_current_minutes(&bytes, request.instrument(), &observed_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SnapshotTransport;
    use magic_market_core::{
        AssetClass, Exchange, InstrumentId, MinuteData, MinuteDataRequest, Money, Quantity,
    };

    fn instrument() -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
    }

    #[test]
    fn keeps_latest_date_and_accumulates_volume_and_amount() {
        let fixture = br#"[
          {"day":"2026-07-22 15:00:00","open":"14.90","high":"14.92","low":"14.90","close":"14.92","volume":"100","amount":"1492"},
          {"day":"2026-07-23 09:30:00","open":"15.30","high":"15.40","low":"15.30","close":"15.40","volume":"1000","amount":"15400"},
          {"day":"2026-07-23 09:31:00","open":"15.40","high":"15.50","low":"15.40","close":"15.50","volume":"2000","amount":"31000"}
        ]"#;
        let batch = parse_current_minutes(fixture, &instrument(), "observed").unwrap();
        assert_eq!(batch.records().len(), 2);
        assert_eq!(
            batch.records()[1].cumulative_quantity(),
            Quantity::new(30.0).unwrap()
        );
        assert_eq!(
            batch.records()[1].cumulative_amount().map(Money::get),
            Some(46_400.0)
        );
        assert_eq!(batch.records()[1].minute_at(), "2026-07-23 09:31");
        assert_eq!(
            batch.provenance().source_at(),
            Some("2026-07-23T09:31:00+08:00")
        );
    }

    #[test]
    fn rejects_missing_amount_unordered_rows_and_overflow() {
        let missing = br#"[{"day":"2026-07-23 09:30:00","open":"1","high":"1","low":"1","close":"1","volume":"1"}]"#;
        assert!(parse_current_minutes(missing, &instrument(), "observed").is_err());
        let unordered = br#"[
          {"day":"2026-07-23 09:31:00","open":"1","high":"1","low":"1","close":"1","volume":"1","amount":"1"},
          {"day":"2026-07-23 09:30:00","open":"1","high":"1","low":"1","close":"1","volume":"1","amount":"1"}
        ]"#;
        assert!(parse_current_minutes(unordered, &instrument(), "observed").is_err());
        let overflow = br#"[
          {"day":"2026-07-23 09:30:00","open":"1","high":"1","low":"1","close":"1","volume":"1e308","amount":"1e308"},
          {"day":"2026-07-23 09:31:00","open":"1","high":"1","low":"1","close":"1","volume":"1e308","amount":"1e308"}
        ]"#;
        assert!(parse_current_minutes(overflow, &instrument(), "observed").is_err());
    }

    struct PanicTransport;

    impl SnapshotTransport for PanicTransport {
        fn get(&self, _url: &str) -> Result<Vec<u8>, SinaError> {
            panic!("historical request must fail before transport")
        }
    }

    #[test]
    fn historical_minute_request_is_explicitly_unsupported() {
        let request = MinuteDataRequest::new(instrument())
            .with_date("2026-07-22")
            .unwrap();
        let client = SinaClient::with_transport(PanicTransport);
        assert!(matches!(
            client.minute_data(&request),
            Err(SinaError::Unsupported(_))
        ));
    }
}
