use crate::{
    instrument_to_thscode, pair_ref, source_millis, validate_identity, validate_safe_text,
    HithinkClient, HithinkError, Success, AUCTION_PATH,
};
use magic_market_core::{
    AssetClass, AuctionSnapshot, DataBatch, DataStatus, FiniteNumber, InstrumentId, Money, Price,
    ProviderId, Quantity, Ratio, RatioUnit,
};
use serde::Deserialize;
use std::collections::HashSet;

const MAX_AUCTION_INSTRUMENTS: usize = 100;
const SHARES_PER_LOT: f64 = 100.0;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuctionData {
    timestamp: i64,
    auction_phase: String,
    data_status: String,
    total: usize,
    item: Vec<AuctionItem>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuctionItem {
    thscode: String,
    ticker: String,
    name: String,
    auction_price: Option<f64>,
    pre_close_price: Option<f64>,
    auction_pct: Option<f64>,
    auction_volume: Option<f64>,
    auction_amount: Option<f64>,
    auction_unmatched: Option<f64>,
    auction_turnover_pct: Option<f64>,
    auction_volume_ratio: Option<f64>,
    auction_yesterday_ratio_pct: Option<f64>,
    float_market_cap: Option<f64>,
    last_price: Option<f64>,
    open_price: Option<f64>,
}

impl HithinkClient {
    /// Fetches the provider's current final auction snapshot without promoting it into exact-date
    /// production routing. Fuyao's response timestamp is observation/assembly time, not source
    /// time, so both record and batch `source_at` deliberately remain absent.
    pub fn probe_auction_snapshots(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<AuctionSnapshot>, HithinkError> {
        let thscodes = validate_request(instruments)?;
        let joined = thscodes.join(",");
        let query = [("thscodes", joined), ("stage", "final".to_owned())];
        let response: Success<AuctionData> = self.get(AUCTION_PATH, query.iter().map(pair_ref))?;
        normalize(instruments, &thscodes, response)
    }
}

fn validate_request(instruments: &[InstrumentId]) -> Result<Vec<String>, HithinkError> {
    if instruments.is_empty() || instruments.len() > MAX_AUCTION_INSTRUMENTS {
        return Err(HithinkError::InvalidRequest(format!(
            "auction request must contain 1..={MAX_AUCTION_INSTRUMENTS} instruments"
        )));
    }
    let mut seen = HashSet::with_capacity(instruments.len());
    let mut thscodes = Vec::with_capacity(instruments.len());
    for instrument in instruments {
        if instrument.asset_class() != AssetClass::Equity {
            return Err(HithinkError::Unsupported(
                "Fuyao auction snapshots support A-share equities only".into(),
            ));
        }
        let thscode = instrument_to_thscode(instrument)?;
        if !seen.insert(thscode.clone()) {
            return Err(HithinkError::InvalidRequest(
                "auction instruments must be unique".into(),
            ));
        }
        thscodes.push(thscode);
    }
    Ok(thscodes)
}

fn normalize(
    instruments: &[InstrumentId],
    thscodes: &[String],
    response: Success<AuctionData>,
) -> Result<DataBatch<AuctionSnapshot>, HithinkError> {
    let observed_at = source_millis(response.data.timestamp)?;
    validate_safe_text("auction_phase", &response.data.auction_phase)?;
    validate_safe_text("data_status", &response.data.data_status)?;
    if response.data.auction_phase != "closed" || response.data.data_status != "final" {
        return Err(HithinkError::Protocol(
            "auction response is not a final closed snapshot".into(),
        ));
    }
    if response.data.total != instruments.len() || response.data.item.len() != instruments.len() {
        return Err(HithinkError::Protocol(
            "auction response does not contain exactly one row per requested instrument".into(),
        ));
    }

    let batch_id = response.request_id;
    let mut records = Vec::with_capacity(instruments.len());
    for ((instrument, expected), item) in instruments.iter().zip(thscodes).zip(response.data.item) {
        validate_identity(expected, instrument.code(), &item.thscode, &item.ticker)?;
        validate_safe_text("auction name", &item.name)?;

        let matched_price = optional_price(item.auction_price, "auction_price")?;
        let previous_close = optional_price(item.pre_close_price, "pre_close_price")?;
        let change_percent =
            optional_ratio(item.auction_pct, "auction_pct", false, RatioUnit::Percent)?;
        let matched_quantity = item
            .auction_volume
            .map(|lots| nonnegative(lots, "auction_volume"))
            .transpose()?
            .map(|lots| Quantity::new(lots * SHARES_PER_LOT))
            .transpose()?;
        let matched_amount = item
            .auction_amount
            .map(|value| nonnegative(value, "auction_amount"))
            .transpose()?
            .map(Money::new)
            .transpose()?;
        // Fuyao exposes one signed aggregate unmatched value without a documented mapping from
        // sign to the two Core queues. Validate it, but never guess which directional slot it
        // belongs to.
        let _unmatched = item.auction_unmatched.map(FiniteNumber::new).transpose()?;
        let _turnover = optional_ratio(
            item.auction_turnover_pct,
            "auction_turnover_pct",
            true,
            RatioUnit::Percent,
        )?;
        let volume_ratio = optional_ratio(
            item.auction_volume_ratio,
            "auction_volume_ratio",
            true,
            RatioUnit::Decimal,
        )?;
        let _yesterday_ratio = optional_ratio(
            item.auction_yesterday_ratio_pct,
            "auction_yesterday_ratio_pct",
            true,
            RatioUnit::Percent,
        )?;
        let _float_market_cap = item
            .float_market_cap
            .map(|value| nonnegative(value, "float_market_cap"))
            .transpose()?;
        let _last_price = optional_price(item.last_price, "last_price")?;
        let _open_price = optional_price(item.open_price, "open_price")?;

        records.push(AuctionSnapshot::new(
            instrument.clone(),
            Some(item.name),
            matched_price,
            previous_close,
            change_percent,
            matched_quantity,
            matched_amount,
            None,
            None,
            volume_ratio,
            DataStatus::Unavailable,
            None,
            observed_at.clone(),
            ProviderId::Tonghuashun,
            batch_id.clone(),
        )?);
    }

    let provenance = magic_market_core::Provenance::new("HithinkFinance", observed_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn optional_price(value: Option<f64>, field: &str) -> Result<Option<Price>, HithinkError> {
    value
        .map(|value| {
            if !value.is_finite() || value <= 0.0 {
                return Err(HithinkError::Protocol(format!(
                    "{field} must be finite and positive"
                )));
            }
            Price::new(value).map_err(Into::into)
        })
        .transpose()
}

fn optional_ratio(
    value: Option<f64>,
    field: &str,
    nonnegative_only: bool,
    unit: RatioUnit,
) -> Result<Option<Ratio>, HithinkError> {
    value
        .map(|value| {
            if !value.is_finite() || (nonnegative_only && value < 0.0) {
                return Err(HithinkError::Protocol(format!(
                    "{field} must be finite{}",
                    if nonnegative_only {
                        " and non-negative"
                    } else {
                        ""
                    }
                )));
            }
            Ratio::new(value, unit).map_err(Into::into)
        })
        .transpose()
}

fn nonnegative(value: f64, field: &str) -> Result<f64, HithinkError> {
    FiniteNumber::new(value)?;
    if value < 0.0 {
        return Err(HithinkError::Protocol(format!(
            "{field} must be non-negative"
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{success, FixtureTransport};
    use magic_market_core::{Auctions, Exchange};
    use serde_json::json;

    fn instrument(code: &str) -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, code, AssetClass::Equity).unwrap()
    }

    fn item(thscode: &str, ticker: &str) -> serde_json::Value {
        json!({
            "thscode": thscode,
            "ticker": ticker,
            "name": "贵州茅台",
            "auction_price": 1688.50,
            "pre_close_price": 1680.00,
            "auction_pct": 0.505952,
            "auction_volume": 12600.0,
            "auction_amount": 2127510000.0,
            "auction_unmatched": -321.0,
            "auction_turnover_pct": 0.012,
            "auction_volume_ratio": 1.25,
            "auction_yesterday_ratio_pct": 101.0,
            "float_market_cap": 2100000000000.0,
            "last_price": 1688.50,
            "open_price": 1688.50
        })
    }

    fn response(items: Vec<serde_json::Value>) -> serde_json::Value {
        let total = items.len();
        success(
            "auction-request",
            json!({
                "timestamp": 1787386686058_i64,
                "auction_phase": "closed",
                "data_status": "final",
                "total": total,
                "item": items
            }),
        )
    }

    #[test]
    fn current_final_snapshot_preserves_observation_semantics_and_lot_unit() {
        let transport = FixtureTransport::new(vec![response(vec![item("600519.SH", "600519")])]);
        let observed = transport.clone();
        let client = HithinkClient::with_transport("test_key", transport).unwrap();

        let batch = client
            .probe_auction_snapshots(&[instrument("600519")])
            .unwrap();

        assert_eq!(batch.records().len(), 1);
        let record = &batch.records()[0];
        assert_eq!(record.instrument(), &instrument("600519"));
        assert_eq!(record.matched_quantity().unwrap().get(), 1_260_000.0);
        assert_eq!(record.matched_amount().unwrap().get(), 2_127_510_000.0);
        assert_eq!(record.volume_ratio().unwrap().unit(), RatioUnit::Decimal);
        assert!(record.unmatched_bid_quantity().is_none());
        assert!(record.unmatched_ask_quantity().is_none());
        assert_eq!(record.status(), DataStatus::Unavailable);
        assert!(record.source_at().is_none());
        assert_eq!(record.observed_at(), "unix-ms:1787386686058");
        assert!(batch.provenance().source_at().is_none());
        assert_eq!(batch.provenance().fetched_at(), "unix-ms:1787386686058");

        let urls = observed.requested_urls();
        assert_eq!(urls.len(), 1);
        assert!(urls[0].contains("/api/a-share/auction/snapshot?"));
        assert!(urls[0].contains("thscodes=600519.SH"));
        assert!(urls[0].contains("stage=final"));
    }

    #[test]
    fn conflicting_identity_or_non_final_state_rejects_the_whole_batch() {
        let client = HithinkClient::with_transport(
            "test_key",
            FixtureTransport::new(vec![response(vec![item("000001.SZ", "000001")])]),
        )
        .unwrap();
        assert!(matches!(
            client.probe_auction_snapshots(&[instrument("600519")]),
            Err(HithinkError::Protocol(_))
        ));

        let client = HithinkClient::with_transport(
            "test_key",
            FixtureTransport::new(vec![success(
                "auction-live",
                json!({
                    "timestamp": 1787386686058_i64,
                    "auction_phase": "matching",
                    "data_status": "live",
                    "total": 1,
                    "item": [item("600519.SH", "600519")]
                }),
            )]),
        )
        .unwrap();
        assert!(matches!(
            client.probe_auction_snapshots(&[instrument("600519")]),
            Err(HithinkError::Protocol(_))
        ));
    }

    #[test]
    fn malformed_unused_numeric_field_is_not_ignored() {
        let mut malformed = item("600519.SH", "600519");
        malformed["float_market_cap"] = json!(-1.0);
        let client = HithinkClient::with_transport(
            "test_key",
            FixtureTransport::new(vec![response(vec![malformed])]),
        )
        .unwrap();
        assert!(matches!(
            client.probe_auction_snapshots(&[instrument("600519")]),
            Err(HithinkError::Protocol(_))
        ));
    }

    #[test]
    fn formal_trait_remains_fail_closed_until_evidence_is_complete() {
        let client =
            HithinkClient::with_transport("test_key", FixtureTransport::default()).unwrap();
        assert!(matches!(
            client.auction_snapshots(&[instrument("600519")]),
            Err(HithinkError::Unsupported(_))
        ));
    }

    #[test]
    fn request_preflight_rejects_empty_duplicate_and_non_equity_without_io() {
        let transport = FixtureTransport::default();
        let observed = transport.clone();
        let client = HithinkClient::with_transport("test_key", transport).unwrap();
        assert!(matches!(
            client.probe_auction_snapshots(&[]),
            Err(HithinkError::InvalidRequest(_))
        ));
        let duplicate = instrument("600519");
        assert!(matches!(
            client.probe_auction_snapshots(&[duplicate.clone(), duplicate]),
            Err(HithinkError::InvalidRequest(_))
        ));
        let index = InstrumentId::new(Exchange::Shanghai, "000001", AssetClass::Index).unwrap();
        assert!(matches!(
            client.probe_auction_snapshots(&[index]),
            Err(HithinkError::Unsupported(_))
        ));
        assert!(observed.requested_urls().is_empty());
    }
}
