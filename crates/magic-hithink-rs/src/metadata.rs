use super::{
    batch_provenance, instrument_to_thscode, now, pair_ref, source_millis, validate_identity,
    validate_safe_text, HithinkClient, HithinkError, Success, SECURITY_METADATA_ADMITTED,
    TICKER_SEARCH_PATH,
};
use magic_market_core::{
    AssetClass, DataBatch, DataStatus, InstrumentId, PriceLimitRule, ProviderId, SecurityMetadata,
    SecurityMetadataProvider,
};
use serde::Deserialize;
use std::collections::HashSet;

const MAX_METADATA_INSTRUMENTS: usize = 32;
const MAX_SEARCH_RESULTS: usize = 50;

impl HithinkClient {
    /// Resolves exact Fuyao identities and names without deriving unavailable listing rules.
    pub fn probe_security_metadata(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityMetadata>, HithinkError> {
        if instruments.is_empty() || instruments.len() > MAX_METADATA_INSTRUMENTS {
            return Err(HithinkError::InvalidRequest(format!(
                "security metadata request must contain 1..={MAX_METADATA_INSTRUMENTS} instruments"
            )));
        }
        let mut seen = HashSet::with_capacity(instruments.len());
        let mut responses = Vec::with_capacity(instruments.len());
        for instrument in instruments {
            if matches!(
                instrument.asset_class(),
                AssetClass::Index | AssetClass::Fund
            ) && instrument.exchange() == magic_market_core::Exchange::Beijing
            {
                return Err(HithinkError::Unsupported(
                    "Fuyao standard-index and exchange-fund metadata requires a Shanghai or Shenzhen identity".into(),
                ));
            }
            let thscode = instrument_to_thscode(instrument)?;
            if !seen.insert(thscode.clone()) {
                return Err(HithinkError::InvalidRequest(
                    "security metadata instruments must be unique".into(),
                ));
            }
            let exchange = thscode[7..].to_owned();
            let asset_type = match instrument.asset_class() {
                AssetClass::Equity => "a-share",
                AssetClass::Index => "a-share-index",
                AssetClass::Fund => "fund-etf,fund-lof,fund-reits",
                _ => {
                    return Err(HithinkError::Unsupported(
                        "Fuyao metadata mapping supports A-share equities, standard indices and exchange-traded funds only".into(),
                    ));
                }
            };
            let query = [
                ("q", thscode),
                ("exchange", exchange),
                ("asset_type", asset_type.to_owned()),
                ("limit", MAX_SEARCH_RESULTS.to_string()),
            ];
            let response = self.get(TICKER_SEARCH_PATH, query.iter().map(pair_ref))?;
            responses.push((instrument.clone(), response));
        }
        normalize_metadata(responses)
    }
}

impl SecurityMetadataProvider for HithinkClient {
    type Error = HithinkError;

    fn security_metadata(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityMetadata>, Self::Error> {
        if SECURITY_METADATA_ADMITTED {
            self.probe_security_metadata(instruments)
        } else {
            Err(HithinkError::Unsupported(
                "HITHINK security metadata awaits production admission".into(),
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TickerSearchData {
    timestamp: i64,
    item: Vec<TickerItem>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TickerItem {
    thscode: String,
    ticker: String,
    name: String,
    exchange: Option<String>,
    asset_type: String,
    currency: String,
}

fn normalize_metadata(
    responses: Vec<(InstrumentId, Success<TickerSearchData>)>,
) -> Result<DataBatch<SecurityMetadata>, HithinkError> {
    let observed_at = now()?;
    let batch_id = metadata_batch_id(&responses)?;
    let latest_timestamp = responses
        .iter()
        .map(|(_, response)| response.data.timestamp)
        .max()
        .ok_or_else(|| HithinkError::Protocol("metadata response set is empty".into()))?;
    let mut records = Vec::with_capacity(responses.len());
    for (instrument, response) in responses {
        if response.data.timestamp <= 0 || response.data.item.len() > MAX_SEARCH_RESULTS {
            return Err(HithinkError::Protocol(
                "metadata search timestamp or result bound is invalid".into(),
            ));
        }
        let expected = instrument_to_thscode(&instrument)?;
        let mut matches = response
            .data
            .item
            .into_iter()
            .filter(|item| item.thscode == expected)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(HithinkError::Protocol(
                "metadata search did not return exactly one exact identity".into(),
            ));
        }
        let item = matches.pop().expect("length checked");
        validate_metadata_identity(&instrument, &expected, &item.thscode, &item.ticker)?;
        validate_safe_text("security name", &item.name)?;
        validate_safe_text("asset_type", &item.asset_type)?;
        validate_safe_text("currency", &item.currency)?;
        let expected_exchange = &expected[7..];
        if item.exchange.as_deref() != Some(expected_exchange)
            || !asset_type_matches(instrument.asset_class(), &item.asset_type)
        {
            return Err(HithinkError::Protocol(
                "metadata identity classification contradicts the request".into(),
            ));
        }
        records.push(SecurityMetadata::new(
            instrument,
            Some(item.name),
            None,
            None,
            None,
            PriceLimitRule::new(None, None)?,
            DataStatus::Unavailable,
            Some(source_millis(response.data.timestamp)?),
            observed_at.clone(),
            ProviderId::Tonghuashun,
            batch_id.clone(),
        )?);
    }
    let provenance = batch_provenance(source_millis(latest_timestamp)?, observed_at, batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn asset_type_matches(asset_class: AssetClass, value: &str) -> bool {
    match asset_class {
        AssetClass::Equity => value == "a-share",
        AssetClass::Index => value == "a-share-index",
        AssetClass::Fund => matches!(value, "fund-etf" | "fund-lof" | "fund-reits"),
        _ => false,
    }
}

fn validate_metadata_identity(
    instrument: &InstrumentId,
    expected_thscode: &str,
    actual_thscode: &str,
    source_ticker: &str,
) -> Result<(), HithinkError> {
    if instrument.asset_class() != AssetClass::Index {
        return validate_identity(
            expected_thscode,
            instrument.code(),
            actual_thscode,
            source_ticker,
        );
    }
    if actual_thscode != expected_thscode
        || source_ticker.is_empty()
        || source_ticker.len() > 32
        || !source_ticker
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err(HithinkError::Protocol(
            "index metadata contains an invalid exact thscode or provider-native ticker".into(),
        ));
    }
    Ok(())
}

fn metadata_batch_id(
    responses: &[(InstrumentId, Success<TickerSearchData>)],
) -> Result<String, HithinkError> {
    let mut value = String::from("hithink-metadata:");
    for (index, (_, response)) in responses.iter().enumerate() {
        if index > 0 {
            value.push(',');
        }
        value.push_str(&response.request_id);
    }
    validate_safe_text("metadata batch_id", &value)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{success, FixtureTransport};
    use magic_market_core::Exchange;
    use serde_json::json;

    fn equity() -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap()
    }

    #[test]
    fn exact_metadata_keeps_name_and_marks_unpublished_fields_unavailable() {
        let timestamp = 1_787_302_800_000_i64;
        let transport = FixtureTransport::new(vec![success(
            "metadata-request",
            json!({
                "timestamp": timestamp,
                "item": [{
                    "thscode": "600519.SH",
                    "ticker": "600519",
                    "name": "贵州茅台",
                    "exchange": "SH",
                    "asset_type": "a-share",
                    "currency": "CNY"
                }]
            }),
        )]);
        let observed = transport.clone();
        let client = HithinkClient::with_transport("test_key", transport).unwrap();
        let batch = client.probe_security_metadata(&[equity()]).unwrap();

        assert!(batch.quality().is_complete());
        assert_eq!(batch.records().len(), 1);
        let record = &batch.records()[0];
        assert_eq!(record.name(), Some("贵州茅台"));
        assert_eq!(record.status(), DataStatus::Unavailable);
        assert_eq!(record.board(), None);
        assert_eq!(record.listed_on(), None);
        assert_eq!(
            record.source_at(),
            Some(format!("unix-ms:{timestamp}").as_str())
        );
        let urls = observed.requested_urls();
        assert!(urls[0].contains(TICKER_SEARCH_PATH));
        assert!(urls[0].contains("q=600519.SH"));
        assert!(urls[0].contains("asset_type=a-share"));
    }

    #[test]
    fn metadata_rejects_ambiguous_exact_identities() {
        let row = json!({
            "thscode": "600519.SH",
            "ticker": "600519",
            "name": "贵州茅台",
            "exchange": "SH",
            "asset_type": "a-share",
            "currency": "CNY"
        });
        let client = HithinkClient::with_transport(
            "test_key",
            FixtureTransport::new(vec![success(
                "metadata-ambiguous",
                json!({"timestamp": 1_787_302_800_000_i64, "item": [row.clone(), row]}),
            )]),
        )
        .unwrap();
        assert!(matches!(
            client.probe_security_metadata(&[equity()]),
            Err(HithinkError::Protocol(_))
        ));
    }

    #[test]
    fn index_metadata_accepts_provider_native_ticker_but_keeps_exact_thscode_identity() {
        let instrument =
            InstrumentId::new(Exchange::Shanghai, "000300", AssetClass::Index).unwrap();
        let client = HithinkClient::with_transport(
            "test_key",
            FixtureTransport::new(vec![success(
                "index-metadata",
                json!({
                    "timestamp": 1_787_356_808_415_i64,
                    "item": [{
                        "thscode": "000300.SH",
                        "ticker": "1B0300",
                        "name": "沪深300",
                        "exchange": "SH",
                        "asset_type": "a-share-index",
                        "currency": "CNY"
                    }]
                }),
            )]),
        )
        .unwrap();
        let batch = client
            .probe_security_metadata(std::slice::from_ref(&instrument))
            .unwrap();
        assert_eq!(batch.records()[0].instrument(), &instrument);
    }

    #[test]
    fn beijing_index_and_fund_metadata_fail_before_transport() {
        let transport = FixtureTransport::default();
        let observed = transport.clone();
        let client = HithinkClient::with_transport("test_key", transport).unwrap();
        for asset_class in [AssetClass::Index, AssetClass::Fund] {
            let instrument = InstrumentId::new(Exchange::Beijing, "920403", asset_class).unwrap();
            assert!(matches!(
                client.probe_security_metadata(&[instrument]),
                Err(HithinkError::Unsupported(_))
            ));
        }
        assert!(observed.requested_urls().is_empty());
    }
}
