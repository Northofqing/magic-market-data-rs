use crate::mapping::{optional_f64, optional_string, optional_u32, required_string};
use crate::post_close::china_now;
use crate::{instrument_from_market, query_url, EastmoneyClient, EastmoneyError};
use magic_market_core::{
    validate_provider_top_n_ranking_batch, DataBatch, FiniteNumber, IsoDate, MarketRankingKind,
    MarketRankingUnit, NonEmptyText, PositiveU32, Provenance, ProviderId,
    ProviderTopNRankingCapabilities, ProviderTopNRankingEntry, ProviderTopNRankingRequest,
    ProviderTopNRankings, SourceEvidence,
};
use ring::digest::{digest, SHA256};
use serde_json::Value;
use std::collections::HashSet;

const PRIMARY_ENDPOINT: &str = "https://push2.eastmoney.com/api/qt/clist/get";
const DELAY_ENDPOINT: &str = "https://push2delay.eastmoney.com/api/qt/clist/get";
const ENDPOINTS: [&str; 2] = [PRIMARY_ENDPOINT, DELAY_ENDPOINT];
const MAX_ENDPOINT_ATTEMPTS: usize = 3;
const TOKEN: &str = "8dec03ba335b81bf4ebdf7b29ec27d15";
const A_SHARE_FILTER: &str =
    "m:0+t:6+f:!2,m:0+t:13+f:!2,m:0+t:80+f:!2,m:1+t:2+f:!2,m:1+t:23+f:!2,m:0+t:81+s:262144+f:!2";
const FIELDS: &str = "f10,f12,f13,f14,f62,f297";
const SOURCE_NAME: &str = "eastmoney-web";

impl ProviderTopNRankings for EastmoneyClient {
    type Error = EastmoneyError;

    fn provider_top_n_rankings(
        &self,
        request: &ProviderTopNRankingRequest,
    ) -> Result<magic_market_core::DataBatch<ProviderTopNRankingEntry>, Self::Error> {
        if !Self::provider_top_n_ranking_capabilities().supports(request.kind()) {
            return Err(EastmoneyError::Unsupported(format!(
                "Eastmoney provider Top-N ranking kind {:?} is not admitted",
                request.kind()
            )));
        }
        self.diagnose_provider_top_n_rankings(request)
    }
}

impl EastmoneyClient {
    /// Builds the only A-share request identity admitted by this provider.
    ///
    /// Consumers must use this constructor rather than copying Eastmoney's
    /// `fs=` wire grammar. The returned identity is still carried by every
    /// row and revalidated at the provider and Router boundaries.
    pub fn provider_top_n_a_share_request(
        kind: MarketRankingKind,
        trading_date: IsoDate,
        limit: PositiveU32,
    ) -> Result<ProviderTopNRankingRequest, EastmoneyError> {
        Ok(ProviderTopNRankingRequest::new(
            kind,
            trading_date,
            limit,
            NonEmptyText::new(A_SHARE_FILTER)?,
        )?)
    }

    /// Exact provenance source identity for the admitted Top-N source.
    pub fn provider_top_n_source_identity() -> Result<NonEmptyText, EastmoneyError> {
        Ok(NonEmptyText::new(SOURCE_NAME)?)
    }

    /// Runs the strict single-response post-close contract without advertising
    /// complete-market coverage.
    pub fn diagnose_provider_top_n_rankings(
        &self,
        request: &ProviderTopNRankingRequest,
    ) -> Result<magic_market_core::DataBatch<ProviderTopNRankingEntry>, EastmoneyError> {
        self.diagnose_provider_top_n_rankings_with_clock(request, china_now)
    }

    fn diagnose_provider_top_n_rankings_with_clock<Clock>(
        &self,
        request: &ProviderTopNRankingRequest,
        mut clock: Clock,
    ) -> Result<magic_market_core::DataBatch<ProviderTopNRankingEntry>, EastmoneyError>
    where
        Clock: FnMut() -> Result<String, EastmoneyError>,
    {
        validate_request(request)?;
        let capture_started_at = clock()?;
        let capture_started_date = validate_capture_observation(request, &capture_started_at)?;
        let field = ranking_field(request.kind())?;
        let mut last_transport_error = None;
        for endpoint in ENDPOINTS {
            let url = provider_top_n_url(endpoint, request, field)?;
            for attempt in 1..=MAX_ENDPOINT_ATTEMPTS {
                match self.get(
                    &url,
                    &[
                        ("Accept", "application/json"),
                        ("Referer", "https://data.eastmoney.com/"),
                    ],
                ) {
                    Ok(bytes) => {
                        let observed_at = clock()?;
                        let capture_completed_date =
                            validate_capture_observation(request, &observed_at)?;
                        if capture_completed_date != capture_started_date {
                            return Err(EastmoneyError::InvalidRequest(format!(
                                "provider Top-N capture crossed China calendar midnight from {} to {}",
                                capture_started_date.as_str(),
                                capture_completed_date.as_str()
                            )));
                        }
                        return parse_provider_top_n(&bytes, request, &observed_at);
                    }
                    Err(EastmoneyError::Transport(message)) => {
                        last_transport_error =
                            Some(format!("{endpoint} attempt {attempt}: {message}"));
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        Err(EastmoneyError::Transport(format!(
            "all Eastmoney provider Top-N HTTPS endpoints failed without one valid response: {}",
            last_transport_error.unwrap_or_else(|| "no endpoint attempted".into())
        )))
    }
}

fn validate_request(request: &ProviderTopNRankingRequest) -> Result<(), EastmoneyError> {
    if request.limit().get() > ProviderTopNRankingRequest::MAX_SINGLE_PAGE_LIMIT {
        return Err(EastmoneyError::InvalidRequest(format!(
            "Eastmoney provider Top-N limit must be at most {}",
            ProviderTopNRankingRequest::MAX_SINGLE_PAGE_LIMIT
        )));
    }
    if request.filter_identity().as_str() != A_SHARE_FILTER {
        return Err(EastmoneyError::InvalidRequest(
            "Eastmoney provider Top-N request filter identity does not match the admitted A-share filter"
                .into(),
        ));
    }
    ranking_field(request.kind()).map(|_| ())
}

fn ranking_field(kind: &MarketRankingKind) -> Result<&'static str, EastmoneyError> {
    match kind {
        MarketRankingKind::VolumeRatio => Ok("f10"),
        MarketRankingKind::MainNetInflow => Ok("f62"),
        other => Err(EastmoneyError::Unsupported(format!(
            "Eastmoney provider Top-N ranking kind {other:?} is not source-proven"
        ))),
    }
}

fn ranking_unit(kind: &MarketRankingKind) -> Result<MarketRankingUnit, EastmoneyError> {
    match kind {
        MarketRankingKind::VolumeRatio => Ok(MarketRankingUnit::Multiple),
        MarketRankingKind::MainNetInflow => Ok(MarketRankingUnit::Yuan),
        other => Err(EastmoneyError::Unsupported(format!(
            "Eastmoney provider Top-N ranking kind {other:?} is not source-proven"
        ))),
    }
}

fn ranking_identity(kind: &MarketRankingKind) -> Result<&'static str, EastmoneyError> {
    match kind {
        MarketRankingKind::VolumeRatio => Ok("volume-ratio"),
        MarketRankingKind::MainNetInflow => Ok("main-net-inflow"),
        other => Err(EastmoneyError::Unsupported(format!(
            "Eastmoney provider Top-N ranking kind {other:?} is not source-proven"
        ))),
    }
}

fn provider_top_n_batch_id(
    request: &ProviderTopNRankingRequest,
    observed_at: &str,
    response: &Value,
) -> Result<String, EastmoneyError> {
    let mut normalized_response = Vec::new();
    write_canonical_json(response, &mut normalized_response);
    Ok(format!(
        "{SOURCE_NAME}:provider-top-n-ranking:v1:{}:{}:{}:{}:{}:{observed_at}",
        ranking_identity(request.kind())?,
        request.trading_date().as_str(),
        request.limit().get(),
        sha256_hex(request.filter_identity().as_str().as_bytes()),
        sha256_hex(&normalized_response),
    ))
}

fn write_canonical_json(value: &Value, output: &mut Vec<u8>) {
    match value {
        Value::Null => output.push(b'n'),
        Value::Bool(value) => {
            output.push(b'b');
            output.push(u8::from(*value));
        }
        Value::Number(value) => write_length_prefixed(b'd', value.to_string().as_bytes(), output),
        Value::String(value) => write_length_prefixed(b's', value.as_bytes(), output),
        Value::Array(values) => {
            write_length_prefixed(b'a', values.len().to_string().as_bytes(), output);
            for value in values {
                write_canonical_json(value, output);
            }
        }
        Value::Object(values) => {
            write_length_prefixed(b'o', values.len().to_string().as_bytes(), output);
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_unstable_by_key(|(key, _)| *key);
            for (key, value) in entries {
                write_length_prefixed(b'k', key.as_bytes(), output);
                write_canonical_json(value, output);
            }
        }
    }
}

fn write_length_prefixed(tag: u8, value: &[u8], output: &mut Vec<u8>) {
    output.push(tag);
    output.extend_from_slice(value.len().to_string().as_bytes());
    output.push(b':');
    output.extend_from_slice(value);
    output.push(b';');
}

fn sha256_hex(input: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = digest(&SHA256, input);
    let mut encoded = String::with_capacity(digest.as_ref().len() * 2);
    for byte in digest.as_ref() {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn provider_top_n_url(
    endpoint: &str,
    request: &ProviderTopNRankingRequest,
    field: &str,
) -> Result<String, EastmoneyError> {
    if !ENDPOINTS.contains(&endpoint) || ranking_field(request.kind())? != field {
        return Err(EastmoneyError::InvalidRequest(
            "unregistered Eastmoney provider Top-N endpoint or metric field".into(),
        ));
    }
    Ok(query_url(
        endpoint,
        &[
            ("pn", "1".into()),
            ("pz", request.limit().get().to_string()),
            ("po", "1".into()),
            ("np", "1".into()),
            ("ut", TOKEN.into()),
            ("fltt", "2".into()),
            ("invt", "2".into()),
            ("fid", field.into()),
            ("fs", request.filter_identity().as_str().into()),
            ("fields", FIELDS.into()),
        ],
    ))
}

fn parse_provider_top_n(
    bytes: &[u8],
    request: &ProviderTopNRankingRequest,
    observed_at: &str,
) -> Result<magic_market_core::DataBatch<ProviderTopNRankingEntry>, EastmoneyError> {
    validate_request(request)?;
    validate_capture_observation(request, observed_at)?;
    let root: Value =
        serde_json::from_slice(bytes).map_err(|error| EastmoneyError::Decode(error.to_string()))?;
    if root.get("rc").and_then(Value::as_i64) != Some(0) {
        return Err(EastmoneyError::Protocol(format!(
            "Eastmoney provider Top-N ranking returned rc {:?}",
            root.get("rc")
        )));
    }
    let data = root
        .get("data")
        .and_then(Value::as_object)
        .ok_or_else(|| EastmoneyError::Protocol("provider Top-N data is absent".into()))?;
    let total = optional_u32(data.get("total"))?
        .filter(|total| *total > 0)
        .ok_or_else(|| EastmoneyError::Protocol("provider Top-N total is absent or zero".into()))?;
    let rows = data
        .get("diff")
        .and_then(Value::as_array)
        .ok_or_else(|| EastmoneyError::Protocol("provider Top-N diff is not an array".into()))?;
    let expected = usize::try_from(request.limit().get().min(total))
        .map_err(|_| EastmoneyError::Protocol("provider Top-N cardinality overflow".into()))?;
    if rows.len() != expected {
        return Err(EastmoneyError::Protocol(format!(
            "provider Top-N returned {} rows but exactly {expected} are required",
            rows.len()
        )));
    }

    let field = ranking_field(request.kind())?;
    let unit = ranking_unit(request.kind())?;
    let batch_id = provider_top_n_batch_id(request, observed_at, &root)?;
    let evidence = SourceEvidence::new(
        ProviderId::Eastmoney,
        observed_at.to_owned(),
        batch_id.clone(),
    )?;
    let inspected_row_count = PositiveU32::new(
        u32::try_from(rows.len())
            .map_err(|_| EastmoneyError::Protocol("provider Top-N row count overflow".into()))?,
    )?;
    let provider_declared_total = PositiveU32::new(total)?;
    let mut instruments = HashSet::with_capacity(rows.len());
    let mut previous = None;
    let mut records = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let code = required_string(row, "f12")?;
        let market = optional_u32(row.get("f13"))?
            .ok_or_else(|| EastmoneyError::Protocol("provider Top-N f13 is absent".into()))?;
        let instrument = instrument_from_market(&code, i64::from(market))?;
        if !instruments.insert(instrument.clone()) {
            return Err(EastmoneyError::Protocol(format!(
                "provider Top-N contains duplicate instrument {code}"
            )));
        }
        let label = NonEmptyText::new(required_string(row, "f14")?)?;
        let value = optional_f64(row.get(field))?
            .ok_or_else(|| EastmoneyError::Protocol(format!("provider Top-N {field} is absent")))?;
        if matches!(request.kind(), MarketRankingKind::VolumeRatio) && value.is_sign_negative() {
            return Err(EastmoneyError::Protocol(
                "provider Top-N volume ratio must be non-negative".into(),
            ));
        }
        if previous.is_some_and(|previous| previous < value) {
            return Err(EastmoneyError::Protocol(format!(
                "provider Top-N {field} is not in descending source order"
            )));
        }
        previous = Some(value);
        let latest_trading_date = parse_latest_trading_date(row.get("f297"))?;
        if &latest_trading_date != request.trading_date() {
            return Err(EastmoneyError::Protocol(format!(
                "provider Top-N latest trading date {} does not match requested date {}",
                latest_trading_date.as_str(),
                request.trading_date().as_str()
            )));
        }
        records.push(ProviderTopNRankingEntry::new(
            request.kind().clone(),
            PositiveU32::new(u32::try_from(index + 1).map_err(|_| {
                EastmoneyError::Protocol("provider Top-N ordinal overflow".into())
            })?)?,
            instrument,
            label,
            FiniteNumber::new(value)?,
            unit.clone(),
            latest_trading_date,
            request.filter_identity().clone(),
            provider_declared_total,
            inspected_row_count,
            evidence.clone(),
        )?);
    }
    let provenance = Provenance::new(SOURCE_NAME, observed_at)?.with_batch_id(batch_id)?;
    let batch = DataBatch::strict(records, provenance);
    validate_provider_top_n_ranking_batch(
        &batch,
        request,
        diagnostic_capabilities(request.kind()),
        ProviderId::Eastmoney,
        &NonEmptyText::new(SOURCE_NAME)?,
    )?;
    Ok(batch)
}

fn diagnostic_capabilities(kind: &MarketRankingKind) -> ProviderTopNRankingCapabilities {
    ProviderTopNRankingCapabilities {
        volume_ratio: matches!(kind, MarketRankingKind::VolumeRatio),
        main_net_inflow: matches!(kind, MarketRankingKind::MainNetInflow),
    }
}

fn parse_latest_trading_date(value: Option<&Value>) -> Result<IsoDate, EastmoneyError> {
    let compact = optional_string(value)?.ok_or_else(|| {
        EastmoneyError::Protocol("provider Top-N f297 latest trading date is absent".into())
    })?;
    if compact.len() != 8 || !compact.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(EastmoneyError::Protocol(format!(
            "provider Top-N f297 {compact:?} must use YYYYMMDD"
        )));
    }
    IsoDate::new(format!(
        "{}-{}-{}",
        &compact[0..4],
        &compact[4..6],
        &compact[6..8]
    ))
    .map_err(|error| {
        EastmoneyError::Protocol(format!(
            "provider Top-N f297 {compact:?} is not a valid calendar date: {error}"
        ))
    })
}

fn validate_capture_observation(
    request: &ProviderTopNRankingRequest,
    observed_at: &str,
) -> Result<IsoDate, EastmoneyError> {
    let local = observed_at.strip_suffix("+08:00").ok_or_else(|| {
        EastmoneyError::InvalidRequest(
            "provider Top-N capture must use an explicit +08:00 observation".into(),
        )
    })?;
    let (date, time) = local.split_once('T').ok_or_else(|| {
        EastmoneyError::InvalidRequest(
            "provider Top-N capture must use YYYY-MM-DDTHH:MM:SS+08:00".into(),
        )
    })?;
    let capture_date = IsoDate::new(date.to_owned()).map_err(|error| {
        EastmoneyError::InvalidRequest(format!(
            "provider Top-N capture has invalid China calendar date: {error}"
        ))
    })?;
    let bytes = time.as_bytes();
    let valid_time_shape = bytes.len() == 8
        && bytes[2] == b':'
        && bytes[5] == b':'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 2 || index == 5 || byte.is_ascii_digit());
    if !valid_time_shape {
        return Err(EastmoneyError::InvalidRequest(
            "provider Top-N capture must use YYYY-MM-DDTHH:MM:SS+08:00".into(),
        ));
    }
    let hour = time[0..2].parse::<u8>().unwrap_or(u8::MAX);
    let minute = time[3..5].parse::<u8>().unwrap_or(u8::MAX);
    let second = time[6..8].parse::<u8>().unwrap_or(u8::MAX);
    if hour > 23 || minute > 59 || second > 59 {
        return Err(EastmoneyError::InvalidRequest(
            "provider Top-N capture has an invalid China clock time".into(),
        ));
    }
    if &capture_date < request.trading_date() {
        return Err(EastmoneyError::InvalidRequest(format!(
            "provider Top-N capture date {} predates requested trading date {}",
            capture_date.as_str(),
            request.trading_date().as_str()
        )));
    }
    if &capture_date == request.trading_date() && time < "15:35:00" {
        return Err(EastmoneyError::InvalidRequest(
            "same-date provider Top-N cannot be captured before 15:35:00 Asia/Shanghai".into(),
        ));
    }
    Ok(capture_date)
}

#[cfg(test)]
#[path = "../tests/internal/provider_top_n_rankings_tests.rs"]
mod tests;
