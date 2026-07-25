use crate::{now, valid_date, valid_time, SinaClient, SinaError, GB18030};
use magic_market_core::{
    DataBatch, FiniteNumber, ForeignExchangeProvider, FxPair, FxQuote, FxRequest, GlobalIndexCode,
    GlobalIndexProvider, GlobalIndexQuote, GlobalIndexRequest, GlobalMarketCapabilities,
    NonEmptyText, Price, Provenance, ProviderId, Ratio, RatioUnit, SourceEvidence,
};
use std::collections::HashMap;

const REFERER: &str = "https://finance.sina.com.cn/";

impl SinaClient {
    /// Capabilities proved for Sina's verified international quote packets.
    pub const fn global_market_capabilities() -> GlobalMarketCapabilities {
        GlobalMarketCapabilities {
            indices: true,
            foreign_exchange: true,
        }
    }

    fn global_packets(&self, symbols: &[&str]) -> Result<HashMap<String, Vec<String>>, SinaError> {
        let bytes = self
            .transport
            .get_with_referer(&format!("{}{}", self.endpoint, symbols.join(",")), REFERER)?;
        parse_packets(&bytes)
    }
}

impl GlobalIndexProvider for SinaClient {
    type Error = SinaError;

    fn global_indices(
        &self,
        request: &GlobalIndexRequest,
    ) -> Result<DataBatch<GlobalIndexQuote>, Self::Error> {
        let symbols: Vec<_> = request
            .indices()
            .iter()
            .copied()
            .map(index_symbol)
            .collect();
        let packets = self.global_packets(&symbols)?;
        ensure_exact_symbols(&symbols, &packets)?;
        let observed_at = now()?;
        let batch_id = format!("sina-web:{observed_at}:global-index");
        let mut records = Vec::with_capacity(symbols.len());
        for (index, symbol) in request.indices().iter().copied().zip(symbols) {
            let fields = packets
                .get(symbol)
                .ok_or_else(|| SinaError::Protocol(format!("response omitted {symbol}")))?;
            if fields.len() != 4 {
                return Err(SinaError::Protocol(format!(
                    "{symbol} has {} fields; exactly 4 are required",
                    fields.len()
                )));
            }
            records.push(GlobalIndexQuote {
                index,
                name: NonEmptyText::new(required_field(&fields[0], "global index name")?)?,
                value: Price::new(parse_positive_field(&fields[1], "global index value")?)?,
                change: FiniteNumber::new(parse_finite_field(&fields[2], "global index change")?)?,
                change_percent: Ratio::new(
                    parse_finite_field(&fields[3], "global index change percent")?,
                    RatioUnit::Percent,
                )?,
                evidence: SourceEvidence::new(
                    ProviderId::Sina,
                    observed_at.clone(),
                    batch_id.clone(),
                )?,
            });
        }
        let provenance = Provenance::new("sina-web", observed_at)?.with_batch_id(batch_id)?;
        Ok(DataBatch::strict(records, provenance))
    }
}

impl ForeignExchangeProvider for SinaClient {
    type Error = SinaError;

    fn foreign_exchange(&self, request: &FxRequest) -> Result<DataBatch<FxQuote>, Self::Error> {
        let symbols: Vec<_> = request.pairs().iter().copied().map(fx_symbol).collect();
        let packets = self.global_packets(&symbols)?;
        ensure_exact_symbols(&symbols, &packets)?;
        let observed_at = now()?;
        let batch_id = format!("sina-web:{observed_at}:foreign-exchange");
        let mut source_times = Vec::with_capacity(symbols.len());
        let mut records = Vec::with_capacity(symbols.len());
        for (pair, symbol) in request.pairs().iter().copied().zip(symbols) {
            let fields = packets
                .get(symbol)
                .ok_or_else(|| SinaError::Protocol(format!("response omitted {symbol}")))?;
            if fields.len() < 18 {
                return Err(SinaError::Protocol(format!(
                    "{symbol} has {} fields; at least 18 are required",
                    fields.len()
                )));
            }
            let date = fields[17].trim();
            let time = fields[0].trim();
            if !valid_date(date) || !valid_time(time) {
                return Err(SinaError::Protocol(format!(
                    "{symbol} has invalid source timestamp {date:?} {time:?}"
                )));
            }
            let source_at = format!("{date}T{time}+08:00");
            source_times.push(source_at.clone());
            records.push(FxQuote {
                pair,
                name: NonEmptyText::new(required_field(&fields[9], "FX name")?)?,
                rate: Price::new(parse_positive_field(&fields[1], "FX rate")?)?,
                change: optional_finite_field(&fields[11], "FX change")?
                    .map(FiniteNumber::new)
                    .transpose()?,
                change_percent: optional_finite_field(&fields[10], "FX change percent")?
                    .map(|value| Ratio::new(value, RatioUnit::Percent))
                    .transpose()?,
                evidence: SourceEvidence::new(
                    ProviderId::Sina,
                    observed_at.clone(),
                    batch_id.clone(),
                )?
                .with_source_at(source_at)?,
            });
        }
        let source_at = source_times
            .iter()
            .min()
            .ok_or_else(|| SinaError::Protocol("empty FX response".into()))?;
        let provenance = Provenance::new("sina-web", observed_at)?
            .with_source_at(source_at.clone())?
            .with_batch_id(batch_id)?;
        Ok(DataBatch::strict(records, provenance))
    }
}

fn index_symbol(index: GlobalIndexCode) -> &'static str {
    match index {
        GlobalIndexCode::DowJones => "int_dji",
        GlobalIndexCode::NasdaqComposite => "int_nasdaq",
        GlobalIndexCode::Sp500 => "int_sp500",
        GlobalIndexCode::Nikkei225 => "int_nikkei",
        GlobalIndexCode::HangSeng => "int_hangseng",
        GlobalIndexCode::Ftse100 => "int_ftse",
    }
}

fn fx_symbol(pair: FxPair) -> &'static str {
    match pair {
        FxPair::UsdCny => "fx_susdcny",
        FxPair::EurUsd => "fx_seurusd",
        FxPair::UsdJpy => "fx_susdjpy",
        FxPair::GbpUsd => "fx_sgbpusd",
        FxPair::AudUsd => "fx_saudusd",
        FxPair::UsdChf => "fx_susdchf",
        FxPair::UsdCad => "fx_susdcad",
        FxPair::NzdUsd => "fx_snzdusd",
    }
}

fn parse_packets(bytes: &[u8]) -> Result<HashMap<String, Vec<String>>, SinaError> {
    if bytes.is_empty() {
        return Err(SinaError::Protocol("empty global quote response".into()));
    }
    let (decoded, _, had_errors) = GB18030.decode(bytes);
    if had_errors {
        return Err(SinaError::Decode(
            "global response contains invalid GB18030 bytes".into(),
        ));
    }
    let mut packets = HashMap::new();
    for line in decoded
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let (variable, encoded) = line.split_once("=\"").ok_or_else(|| {
            SinaError::Protocol("global line is missing the opening delimiter".into())
        })?;
        let symbol = variable
            .strip_prefix("var hq_str_")
            .filter(|symbol| symbol.starts_with("int_") || symbol.starts_with("fx_s"))
            .ok_or_else(|| SinaError::Protocol("global line has an invalid symbol key".into()))?;
        let encoded = encoded.strip_suffix("\";").ok_or_else(|| {
            SinaError::Protocol("global line is missing the closing delimiter".into())
        })?;
        if encoded.is_empty() {
            return Err(SinaError::Protocol(format!(
                "{symbol} returned an empty packet"
            )));
        }
        let fields = encoded.split(',').map(str::to_owned).collect();
        if packets.insert(symbol.to_owned(), fields).is_some() {
            return Err(SinaError::Protocol(format!(
                "duplicate global response record {symbol}"
            )));
        }
    }
    if packets.is_empty() {
        return Err(SinaError::Protocol(
            "response did not contain global quote records".into(),
        ));
    }
    Ok(packets)
}

fn ensure_exact_symbols(
    symbols: &[&str],
    packets: &HashMap<String, Vec<String>>,
) -> Result<(), SinaError> {
    if packets.len() != symbols.len() {
        return Err(SinaError::Protocol(format!(
            "global response cardinality mismatch: requested {}, received {}",
            symbols.len(),
            packets.len()
        )));
    }
    if symbols.iter().any(|symbol| !packets.contains_key(*symbol)) {
        return Err(SinaError::Protocol(
            "global response omitted a requested identity".into(),
        ));
    }
    Ok(())
}

fn required_field(value: &str, field: &str) -> Result<String, SinaError> {
    let value = value.trim();
    if value.is_empty() {
        Err(SinaError::Protocol(format!("{field} is missing")))
    } else {
        Ok(value.to_owned())
    }
}

fn parse_finite_field(value: &str, field: &str) -> Result<f64, SinaError> {
    let value = required_field(value, field)?;
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| SinaError::Protocol(format!("{field} is not finite: {value:?}")))
}

fn parse_positive_field(value: &str, field: &str) -> Result<f64, SinaError> {
    let value = parse_finite_field(value, field)?;
    if value <= 0.0 {
        return Err(SinaError::Protocol(format!("{field} must be positive")));
    }
    Ok(value)
}

fn optional_finite_field(value: &str, field: &str) -> Result<Option<f64>, SinaError> {
    if value.trim().is_empty() {
        Ok(None)
    } else {
        parse_finite_field(value, field).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SnapshotTransport;
    use encoding_rs::GB18030;

    struct FixtureTransport {
        response: Vec<u8>,
    }

    impl SnapshotTransport for FixtureTransport {
        fn get(&self, _url: &str) -> Result<Vec<u8>, SinaError> {
            Ok(self.response.clone())
        }
    }

    fn encoded(value: &str) -> Vec<u8> {
        let (encoded, _, _) = GB18030.encode(value);
        encoded.into_owned()
    }

    #[test]
    fn parses_verified_global_index_packet() {
        let client = SinaClient::with_transport(FixtureTransport {
            response: encoded(
                "var hq_str_int_dji=\"道琼斯,44901.92,-81.08,-0.18\";\n\
                 var hq_str_int_sp500=\"标普500,6388.64,4.01,0.06\";",
            ),
        });
        let request =
            GlobalIndexRequest::new(vec![GlobalIndexCode::DowJones, GlobalIndexCode::Sp500])
                .unwrap();
        let batch = client.global_indices(&request).unwrap();
        assert_eq!(batch.records().len(), 2);
        assert_eq!(batch.records()[0].value.get(), 44_901.92);
        assert!(batch.provenance().source_at().is_none());
    }

    #[test]
    fn parses_verified_fx_packet_and_timestamp() {
        let fields = [
            "15:30:00",
            "7.1689",
            "7.1690",
            "7.1691",
            "7.1700",
            "7.1600",
            "7.1680",
            "7.1692",
            "7.1688",
            "美元人民币",
            "0.10",
            "0.0071",
            "0",
            "0",
            "0",
            "0",
            "0",
            "2026-07-24",
        ];
        let response = format!("var hq_str_fx_susdcny=\"{}\";", fields.join(","));
        let client = SinaClient::with_transport(FixtureTransport {
            response: encoded(&response),
        });
        let request = FxRequest::new(vec![FxPair::UsdCny]).unwrap();
        let batch = client.foreign_exchange(&request).unwrap();
        assert_eq!(batch.records()[0].rate.get(), 7.1689);
        assert_eq!(
            batch.records()[0].evidence.source_at(),
            Some("2026-07-24T15:30:00+08:00")
        );
    }

    #[test]
    fn rejects_empty_duplicate_and_unexpected_packets() {
        assert!(parse_packets(&[]).is_err());
        let duplicate =
            "var hq_str_int_dji=\"道琼斯,1,0,0\";\nvar hq_str_int_dji=\"道琼斯,1,0,0\";";
        assert!(parse_packets(&encoded(duplicate)).is_err());
        let client = SinaClient::with_transport(FixtureTransport {
            response: encoded("var hq_str_int_sp500=\"标普500,1,0,0\";"),
        });
        let request = GlobalIndexRequest::new(vec![GlobalIndexCode::DowJones]).unwrap();
        assert!(client.global_indices(&request).is_err());
    }
}
