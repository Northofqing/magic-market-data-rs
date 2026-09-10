use crate::{
    instrument_to_thscode, pair_ref, source_millis, validate_identity, validate_safe_text,
    HithinkClient, HithinkError, Success, AUCTION_PATH,
};
use magic_market_core::{
    AssetClass, AuctionSnapshot, DataBatch, DataStatus, FiniteNumber, InstrumentId, Money, Price,
    ProviderId, Quantity, Ratio, RatioUnit, SourceEvidence, SourcedRecord,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const MAX_AUCTION_INSTRUMENTS: usize = 100;
const SHARES_PER_LOT: f64 = 100.0;

/// Caller-selected view of the provider's current, date-less auction snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurrentAuctionStage {
    Live,
    Final,
}

impl CurrentAuctionStage {
    fn as_query_value(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Final => "final",
        }
    }
}

/// Provider-declared availability state for a returned auction observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CurrentAuctionDataStatus {
    Live,
    Final,
    Suspended,
}

/// Truthful projection of one HITHINK current auction row.
///
/// This deliberately has no trading date, provider event time, or directional
/// unmatched queues because the upstream snapshot publishes none of them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CurrentAuctionObservation {
    instrument: InstrumentId,
    name: String,
    requested_stage: CurrentAuctionStage,
    auction_phase: String,
    data_status: CurrentAuctionDataStatus,
    auction_price: Option<Price>,
    pre_close_price: Option<Price>,
    auction_pct: Option<Ratio>,
    auction_volume_shares: Option<Quantity>,
    auction_amount: Option<Money>,
    /// Signed provider-native value. The source does not publish its unit or
    /// define which sign represents the bid or ask side.
    auction_unmatched: Option<FiniteNumber>,
    auction_turnover_pct: Option<Ratio>,
    auction_volume_ratio: Option<Ratio>,
    auction_yesterday_ratio_pct: Option<Ratio>,
    float_market_cap: Option<Money>,
    last_price: Option<Price>,
    open_price: Option<Price>,
    evidence: SourceEvidence,
}

impl CurrentAuctionObservation {
    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn requested_stage(&self) -> CurrentAuctionStage {
        self.requested_stage
    }

    pub fn auction_phase(&self) -> &str {
        &self.auction_phase
    }

    pub fn data_status(&self) -> CurrentAuctionDataStatus {
        self.data_status
    }

    pub fn auction_price(&self) -> Option<Price> {
        self.auction_price
    }

    pub fn pre_close_price(&self) -> Option<Price> {
        self.pre_close_price
    }

    pub fn auction_pct(&self) -> Option<Ratio> {
        self.auction_pct
    }

    pub fn auction_volume_shares(&self) -> Option<Quantity> {
        self.auction_volume_shares
    }

    pub fn auction_amount(&self) -> Option<Money> {
        self.auction_amount
    }

    pub fn auction_unmatched(&self) -> Option<FiniteNumber> {
        self.auction_unmatched
    }

    pub fn auction_turnover_pct(&self) -> Option<Ratio> {
        self.auction_turnover_pct
    }

    pub fn auction_volume_ratio(&self) -> Option<Ratio> {
        self.auction_volume_ratio
    }

    pub fn auction_yesterday_ratio_pct(&self) -> Option<Ratio> {
        self.auction_yesterday_ratio_pct
    }

    pub fn float_market_cap(&self) -> Option<Money> {
        self.float_market_cap
    }

    pub fn last_price(&self) -> Option<Price> {
        self.last_price
    }

    pub fn open_price(&self) -> Option<Price> {
        self.open_price
    }

    pub fn evidence(&self) -> &SourceEvidence {
        &self.evidence
    }

    fn into_legacy_snapshot(self) -> Result<AuctionSnapshot, HithinkError> {
        AuctionSnapshot::new(
            self.instrument,
            Some(self.name),
            self.auction_price,
            self.pre_close_price,
            self.auction_pct,
            self.auction_volume_shares,
            self.auction_amount,
            None,
            None,
            self.auction_volume_ratio,
            DataStatus::Unavailable,
            None,
            self.evidence.observed_at(),
            self.evidence.provider(),
            self.evidence.batch_id(),
        )
        .map_err(Into::into)
    }
}

impl SourcedRecord for CurrentAuctionObservation {
    fn provider_id(&self) -> ProviderId {
        self.evidence.provider()
    }

    fn evidence_batch_id(&self) -> &str {
        self.evidence.batch_id()
    }
}

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
    /// Fetches one current auction observation per requested instrument.
    ///
    /// The returned evidence contains observation time only. It must not be
    /// used as exact-date or provider-source-time auction evidence.
    pub fn current_auction_observations(
        &self,
        instruments: &[InstrumentId],
        stage: CurrentAuctionStage,
    ) -> Result<DataBatch<CurrentAuctionObservation>, HithinkError> {
        let thscodes = validate_request(instruments)?;
        let joined = thscodes.join(",");
        let query = [
            ("thscodes", joined),
            ("stage", stage.as_query_value().to_owned()),
        ];
        let response: Success<AuctionData> = self.get(AUCTION_PATH, query.iter().map(pair_ref))?;
        normalize_observations(instruments, &thscodes, stage, response)
    }

    /// Fetches the provider's current final auction snapshot without promoting it into exact-date
    /// production routing. Fuyao's response timestamp is observation/assembly time, not source
    /// time, so both record and batch `source_at` deliberately remain absent.
    pub fn probe_auction_snapshots(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<AuctionSnapshot>, HithinkError> {
        let batch = self.current_auction_observations(instruments, CurrentAuctionStage::Final)?;
        let provenance = batch.provenance().clone();
        let records = batch
            .into_records()
            .into_iter()
            .map(CurrentAuctionObservation::into_legacy_snapshot)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DataBatch::strict(records, provenance))
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

fn normalize_observations(
    instruments: &[InstrumentId],
    thscodes: &[String],
    requested_stage: CurrentAuctionStage,
    response: Success<AuctionData>,
) -> Result<DataBatch<CurrentAuctionObservation>, HithinkError> {
    let observed_at = source_millis(response.data.timestamp)?;
    validate_safe_text("auction_phase", &response.data.auction_phase)?;
    validate_safe_text("data_status", &response.data.data_status)?;
    if response.data.data_status == "not_ready" {
        return Err(HithinkError::NotReady {
            request_id: response.request_id,
        });
    }
    let data_status = match response.data.data_status.as_str() {
        "live" => CurrentAuctionDataStatus::Live,
        "final" => CurrentAuctionDataStatus::Final,
        "suspended" => CurrentAuctionDataStatus::Suspended,
        _ => {
            return Err(HithinkError::Protocol(
                "auction response has an unknown data_status".into(),
            ))
        }
    };
    let state_matches_request = matches!(
        (requested_stage, data_status),
        (CurrentAuctionStage::Live, CurrentAuctionDataStatus::Live)
            | (CurrentAuctionStage::Final, CurrentAuctionDataStatus::Final)
            | (_, CurrentAuctionDataStatus::Suspended)
    );
    if !state_matches_request
        || (data_status == CurrentAuctionDataStatus::Final
            && response.data.auction_phase != "closed")
    {
        return Err(HithinkError::Protocol(
            "auction response state contradicts the requested stage".into(),
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

        let auction_price = optional_observed_price(item.auction_price, "auction_price")?;
        let pre_close_price = optional_observed_price(item.pre_close_price, "pre_close_price")?;
        let auction_pct =
            optional_ratio(item.auction_pct, "auction_pct", false, RatioUnit::Percent)?;
        let auction_volume_shares = item
            .auction_volume
            .map(|lots| nonnegative(lots, "auction_volume"))
            .transpose()?
            .map(|lots| Quantity::new(lots * SHARES_PER_LOT))
            .transpose()?;
        let auction_amount = item
            .auction_amount
            .map(|value| nonnegative(value, "auction_amount"))
            .transpose()?
            .map(Money::new)
            .transpose()?;
        let auction_unmatched = item.auction_unmatched.map(FiniteNumber::new).transpose()?;
        let auction_turnover_pct = optional_ratio(
            item.auction_turnover_pct,
            "auction_turnover_pct",
            true,
            RatioUnit::Percent,
        )?;
        let auction_volume_ratio = optional_ratio(
            item.auction_volume_ratio,
            "auction_volume_ratio",
            true,
            RatioUnit::Decimal,
        )?;
        let auction_yesterday_ratio_pct = optional_ratio(
            item.auction_yesterday_ratio_pct,
            "auction_yesterday_ratio_pct",
            true,
            RatioUnit::Percent,
        )?;
        let float_market_cap = item
            .float_market_cap
            .map(|value| nonnegative(value, "float_market_cap"))
            .transpose()?
            .map(Money::new)
            .transpose()?;
        let last_price = optional_observed_price(item.last_price, "last_price")?;
        let open_price = optional_observed_price(item.open_price, "open_price")?;
        let evidence = SourceEvidence::new(
            ProviderId::Tonghuashun,
            observed_at.clone(),
            batch_id.clone(),
        )?;

        records.push(CurrentAuctionObservation {
            instrument: instrument.clone(),
            name: item.name,
            requested_stage,
            auction_phase: response.data.auction_phase.clone(),
            data_status,
            auction_price,
            pre_close_price,
            auction_pct,
            auction_volume_shares,
            auction_amount,
            auction_unmatched,
            auction_turnover_pct,
            auction_volume_ratio,
            auction_yesterday_ratio_pct,
            float_market_cap,
            last_price,
            open_price,
            evidence,
        });
    }

    let provenance = magic_market_core::Provenance::new("HithinkFinance", observed_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn optional_observed_price(value: Option<f64>, field: &str) -> Result<Option<Price>, HithinkError> {
    value
        .map(|value| {
            if !value.is_finite() || value < 0.0 {
                return Err(HithinkError::Protocol(format!(
                    "{field} must be finite and non-negative"
                )));
            }
            if value == 0.0 {
                Ok(None)
            } else {
                Price::new(value).map(Some).map_err(Into::into)
            }
        })
        .transpose()
        .map(Option::flatten)
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
    fn closed_not_ready_state_is_a_typed_terminal_without_records() {
        let client = HithinkClient::with_transport(
            "test_key",
            FixtureTransport::new(vec![success(
                "auction-not-ready",
                json!({
                    "timestamp": 1787386686058_i64,
                    "auction_phase": "closed",
                    "data_status": "not_ready",
                    "total": 1,
                    "item": [item("600519.SH", "600519")]
                }),
            )]),
        )
        .unwrap();

        assert!(matches!(
            client.probe_auction_snapshots(&[instrument("600519")]),
            Err(HithinkError::NotReady { request_id }) if request_id == "auction-not-ready"
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
    fn unknown_provider_field_rejects_the_whole_observation_batch() {
        let mut changed = item("600519.SH", "600519");
        changed["undocumented_queue_side"] = json!("buy");
        let client = HithinkClient::with_transport(
            "test_key",
            FixtureTransport::new(vec![response(vec![changed])]),
        )
        .unwrap();

        assert!(matches!(
            client
                .current_auction_observations(&[instrument("600519")], CurrentAuctionStage::Final),
            Err(HithinkError::Decode(_))
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
    fn live_observation_preserves_unmatched_and_accepts_no_trade_zeroes() {
        let mut no_trade = item("600519.SH", "600519");
        no_trade["auction_price"] = json!(0.0);
        no_trade["auction_volume"] = json!(0.0);
        no_trade["auction_amount"] = json!(0.0);
        no_trade["last_price"] = json!(0.0);
        no_trade["open_price"] = json!(0.0);
        let transport = FixtureTransport::new(vec![success(
            "auction-live-zero",
            json!({
                "timestamp": 1787386686058_i64,
                "auction_phase": "matching",
                "data_status": "live",
                "total": 1,
                "item": [no_trade]
            }),
        )]);
        let observed = transport.clone();
        let client = HithinkClient::with_transport("test_key", transport).unwrap();

        let batch = client
            .current_auction_observations(&[instrument("600519")], CurrentAuctionStage::Live)
            .unwrap();

        assert_eq!(batch.records().len(), 1);
        let record = &batch.records()[0];
        assert_eq!(record.requested_stage(), CurrentAuctionStage::Live);
        assert_eq!(record.auction_phase(), "matching");
        assert_eq!(record.data_status(), CurrentAuctionDataStatus::Live);
        assert!(record.auction_price().is_none());
        assert_eq!(record.auction_volume_shares().unwrap().get(), 0.0);
        assert_eq!(record.auction_amount().unwrap().get(), 0.0);
        assert_eq!(record.auction_unmatched().unwrap().get(), -321.0);
        assert!(record.last_price().is_none());
        assert!(record.open_price().is_none());
        assert!(record.evidence().source_at().is_none());
        assert_eq!(record.evidence().observed_at(), "unix-ms:1787386686058");
        assert_eq!(record.evidence().batch_id(), "auction-live-zero");
        assert!(batch.provenance().source_at().is_none());

        let json = serde_json::to_value(record).unwrap();
        assert!(json.get("trading_date").is_none());
        assert!(json["evidence"]["source_at"].is_null());

        let urls = observed.requested_urls();
        assert_eq!(urls.len(), 1);
        assert!(urls[0].contains("stage=live"));
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
