use crate::mapping::{optional_f64, optional_u32, required_string};
use crate::{instrument_from_market, query_url, EastmoneyClient, EastmoneyError};
use magic_market_core::{
    unix_seconds_to_china_rfc3339, ClockTime, DataBatch, InstrumentId, IsoDate, Money,
    NonEmptyText, PositiveU32, PostCloseFlow, PostCloseFlowRequest, PostCloseFlows, Price,
    Provenance, ProviderId, Ratio, RatioUnit, SourceEvidence,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

const PRIMARY_ENDPOINT: &str = "https://push2.eastmoney.com/api/qt/clist/get";
const DELAY_ENDPOINT: &str = "https://push2delay.eastmoney.com/api/qt/clist/get";
const ENDPOINTS: [&str; 2] = [PRIMARY_ENDPOINT, DELAY_ENDPOINT];
const MAX_ENDPOINT_ATTEMPTS: usize = 3;
const TOKEN: &str = "8dec03ba335b81bf4ebdf7b29ec27d15";
const A_SHARE_FILTER: &str =
    "m:0+t:6+f:!2,m:0+t:13+f:!2,m:0+t:80+f:!2,m:1+t:2+f:!2,m:1+t:23+f:!2,m:0+t:81+s:262144+f:!2";
const FIELDS: &str = "f1,f2,f3,f12,f13,f14,f62,f184,f124";

/// A bounded post-close diagnostic row. Missing source values remain `null`;
/// the type intentionally does not satisfy the strict production contract.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiagnosticPostCloseFlow {
    instrument: Option<InstrumentId>,
    name: Option<NonEmptyText>,
    trading_date: IsoDate,
    source_rank: PositiveU32,
    close: Option<Price>,
    change_percent: Option<Ratio>,
    main_net: Option<Money>,
    main_net_percent: Option<Ratio>,
    super_large_net: Option<Money>,
    large_net: Option<Money>,
    reported_total: PositiveU32,
    source_at: Option<String>,
    evidence: SourceEvidence,
}

impl PostCloseFlows for EastmoneyClient {
    type Error = EastmoneyError;

    fn post_close_flows(
        &self,
        _request: &PostCloseFlowRequest,
    ) -> Result<DataBatch<PostCloseFlow>, Self::Error> {
        Err(EastmoneyError::Unsupported(
            "Eastmoney strict post-close flow has not passed production admission; use diagnose_post_close_flows for bounded diagnostics".into(),
        ))
    }
}

impl EastmoneyClient {
    /// Runs the strict current-day source contract without advertising it as an
    /// admitted production capability.
    ///
    /// The diagnostic still rejects partial universes, missing names, mixed
    /// provider timestamps, and pre-15:35 captures. A successful diagnostic is
    /// evidence for operator review, not automatic capability admission.
    pub fn diagnose_post_close_flows(
        &self,
        request: &PostCloseFlowRequest,
    ) -> Result<DataBatch<PostCloseFlow>, EastmoneyError> {
        self.diagnose_post_close_flows_with_clock(request, china_now)
    }

    /// Returns the same bounded current-day observation while retaining
    /// per-record source times when the provider response is not atomic.
    /// Mixed source times force a best-effort batch and never enable the
    /// production `PostCloseFlows` capability.
    pub fn diagnose_partial_post_close_flows(
        &self,
        request: &PostCloseFlowRequest,
    ) -> Result<DataBatch<DiagnosticPostCloseFlow>, EastmoneyError> {
        let capture_started_at = china_now()?;
        validate_partial_capture_window(request, &capture_started_at)?;
        let limit = request.limit().get();
        let mut last_transport_error = None;
        for endpoint in ENDPOINTS {
            let url = post_close_url(endpoint, limit)?;
            for attempt in 1..=MAX_ENDPOINT_ATTEMPTS {
                match self.get(
                    &url,
                    &[
                        ("Accept", "application/json"),
                        ("Referer", "https://data.eastmoney.com/zjlx/list.html"),
                    ],
                ) {
                    Ok(bytes) => {
                        let observed_at = china_now()?;
                        return parse_available_post_close(&bytes, request, &observed_at);
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
            "all Eastmoney partial post-close HTTPS endpoints failed: {}",
            last_transport_error.unwrap_or_else(|| "no endpoint attempted".into())
        )))
    }

    fn diagnose_post_close_flows_with_clock<Clock>(
        &self,
        request: &PostCloseFlowRequest,
        clock: Clock,
    ) -> Result<DataBatch<PostCloseFlow>, EastmoneyError>
    where
        Clock: Fn() -> Result<String, EastmoneyError>,
    {
        self.diagnose_post_close_flows_mode(request, clock, false)
    }

    fn diagnose_post_close_flows_mode<Clock>(
        &self,
        request: &PostCloseFlowRequest,
        clock: Clock,
        allow_mixed_source_times: bool,
    ) -> Result<DataBatch<PostCloseFlow>, EastmoneyError>
    where
        Clock: Fn() -> Result<String, EastmoneyError>,
    {
        let capture_started_at = clock()?;
        validate_capture_window(request, &capture_started_at)?;
        let limit = request.limit().get();
        let mut last_transport_error = None;
        for endpoint in ENDPOINTS {
            let url = post_close_url(endpoint, limit)?;
            for attempt in 1..=MAX_ENDPOINT_ATTEMPTS {
                match self.get(
                    &url,
                    &[
                        ("Accept", "application/json"),
                        ("Referer", "https://data.eastmoney.com/zjlx/list.html"),
                    ],
                ) {
                    Ok(bytes) => {
                        let observed_at = clock()?;
                        return parse_post_close_mode(
                            &bytes,
                            request,
                            &observed_at,
                            allow_mixed_source_times,
                        );
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
            "all Eastmoney post-close HTTPS endpoints failed without a complete snapshot: {}",
            last_transport_error.unwrap_or_else(|| "no endpoint attempted".into())
        )))
    }
}

fn post_close_url(endpoint: &str, limit: u32) -> Result<String, EastmoneyError> {
    if !ENDPOINTS.contains(&endpoint) || limit == 0 {
        return Err(EastmoneyError::InvalidRequest(
            "unregistered Eastmoney post-close endpoint or zero limit".into(),
        ));
    }
    Ok(query_url(
        endpoint,
        &[
            ("pn", "1".into()),
            ("pz", limit.to_string()),
            ("po", "1".into()),
            ("np", "1".into()),
            ("ut", TOKEN.into()),
            ("fltt", "2".into()),
            ("invt", "2".into()),
            ("fid", "f62".into()),
            ("fs", A_SHARE_FILTER.into()),
            ("fields", FIELDS.into()),
        ],
    ))
}

fn parse_available_post_close(
    bytes: &[u8],
    request: &PostCloseFlowRequest,
    observed_at: &str,
) -> Result<DataBatch<DiagnosticPostCloseFlow>, EastmoneyError> {
    validate_partial_capture_window(request, observed_at)?;
    let root: Value =
        serde_json::from_slice(bytes).map_err(|error| EastmoneyError::Decode(error.to_string()))?;
    if root.get("rc").and_then(Value::as_i64) != Some(0) {
        return Err(EastmoneyError::Protocol(format!(
            "post-close ranking returned rc {:?}",
            root.get("rc")
        )));
    }
    let data = root
        .get("data")
        .and_then(Value::as_object)
        .ok_or_else(|| EastmoneyError::Protocol("post-close data is absent".into()))?;
    let total = optional_u32(data.get("total"))?
        .ok_or_else(|| EastmoneyError::Protocol("post-close total is absent".into()))?;
    if total < request.limit().get() {
        return Err(EastmoneyError::Protocol(format!(
            "post-close source total {total} is below requested limit {}",
            request.limit().get()
        )));
    }
    let rows = data
        .get("diff")
        .and_then(Value::as_array)
        .ok_or_else(|| EastmoneyError::Protocol("post-close diff is not an array".into()))?;
    if rows.len() != request.limit().get() as usize {
        return Err(EastmoneyError::Protocol(format!(
            "post-close ranking returned {} rows for requested limit {}",
            rows.len(),
            request.limit().get()
        )));
    }

    let batch_id = format!(
        "eastmoney-web:post-close-flow-diagnostic:{}:{observed_at}",
        request.trading_date().as_str()
    );
    let mut instruments = HashSet::with_capacity(rows.len());
    let mut missing_fields = 0_u32;
    let mut records = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let code = optional_nonempty(row.get("f12"))?;
        let market = optional_u32(row.get("f13"))?;
        let instrument = match (code.as_ref(), market) {
            (Some(code), Some(market)) => {
                let instrument = instrument_from_market(code.as_str(), i64::from(market))?;
                if !instruments.insert(instrument.clone()) {
                    return Err(EastmoneyError::Protocol(format!(
                        "post-close ranking contains duplicate instrument {}",
                        code.as_str()
                    )));
                }
                Some(instrument)
            }
            _ => {
                missing_fields = missing_fields.saturating_add(1);
                None
            }
        };
        let name = optional_nonempty(row.get("f14"))?;
        if name.is_none() {
            missing_fields = missing_fields.saturating_add(1);
        }
        let close = optional_f64(row.get("f2"))?.map(Price::new).transpose()?;
        let change_percent = optional_f64(row.get("f3"))?
            .map(|value| Ratio::new(value, RatioUnit::Percent))
            .transpose()?;
        let main_net = optional_f64(row.get("f62"))?.map(Money::new).transpose()?;
        let main_net_percent = optional_f64(row.get("f184"))?
            .map(|value| Ratio::new(value, RatioUnit::Percent))
            .transpose()?;
        for value in [
            close.is_some(),
            change_percent.is_some(),
            main_net.is_some(),
            main_net_percent.is_some(),
        ] {
            if !value {
                missing_fields = missing_fields.saturating_add(1);
            }
        }
        let source_at = optional_u32(row.get("f124"))?
            .filter(|epoch| *epoch > 0)
            .map(|epoch| {
                unix_seconds_to_china_rfc3339(i64::from(epoch))
                    .map_err(|_| EastmoneyError::Protocol("post-close f124 is out of range".into()))
            })
            .transpose()?;
        if let Some(source_at) = &source_at {
            if source_at.get(..10) != Some(request.trading_date().as_str()) {
                return Err(EastmoneyError::Protocol(format!(
                    "post-close source timestamp {source_at} does not match requested date {}",
                    request.trading_date().as_str()
                )));
            }
        } else {
            missing_fields = missing_fields.saturating_add(1);
        }
        let evidence = SourceEvidence::new(
            ProviderId::Eastmoney,
            observed_at.to_owned(),
            batch_id.clone(),
        )?;
        let evidence = match &source_at {
            Some(source_at) => evidence.with_source_at(source_at.clone())?,
            None => evidence,
        };
        records.push(DiagnosticPostCloseFlow {
            instrument,
            name,
            trading_date: request.trading_date().clone(),
            source_rank: PositiveU32::new(
                u32::try_from(index + 1)
                    .map_err(|_| EastmoneyError::Protocol("post-close rank overflow".into()))?,
            )?,
            close,
            change_percent,
            main_net,
            main_net_percent,
            super_large_net: None,
            large_net: None,
            reported_total: PositiveU32::new(total)?,
            source_at,
            evidence,
        });
    }
    let provenance = Provenance::new("eastmoney-web", observed_at)?.with_batch_id(batch_id)?;
    let mut issues = vec![
        "diagnostic post-close rows are not a complete atomic market snapshot".to_owned(),
        "super_large_net and large_net are not supplied by this source contract and remain null"
            .to_owned(),
    ];
    if missing_fields > 0 {
        issues.push(format!(
            "{missing_fields} optional source fields were absent and remain null"
        ));
    }
    Ok(DataBatch::best_effort(records, provenance, issues)?)
}

#[cfg(test)]
fn parse_post_close(
    bytes: &[u8],
    request: &PostCloseFlowRequest,
    observed_at: &str,
) -> Result<DataBatch<PostCloseFlow>, EastmoneyError> {
    parse_post_close_mode(bytes, request, observed_at, false)
}

fn parse_post_close_mode(
    bytes: &[u8],
    request: &PostCloseFlowRequest,
    observed_at: &str,
    allow_mixed_source_times: bool,
) -> Result<DataBatch<PostCloseFlow>, EastmoneyError> {
    validate_capture_window(request, observed_at)?;
    let root: Value =
        serde_json::from_slice(bytes).map_err(|error| EastmoneyError::Decode(error.to_string()))?;
    if root.get("rc").and_then(Value::as_i64) != Some(0) {
        return Err(EastmoneyError::Protocol(format!(
            "post-close ranking returned rc {:?}",
            root.get("rc")
        )));
    }
    let data = root
        .get("data")
        .and_then(Value::as_object)
        .ok_or_else(|| EastmoneyError::Protocol("post-close data is absent".into()))?;
    let total = optional_u32(data.get("total"))?
        .ok_or_else(|| EastmoneyError::Protocol("post-close total is absent".into()))?;
    if total < request.limit().get() {
        return Err(EastmoneyError::Protocol(format!(
            "post-close source total {total} is below requested limit {}",
            request.limit().get()
        )));
    }
    let rows = data
        .get("diff")
        .and_then(Value::as_array)
        .ok_or_else(|| EastmoneyError::Protocol("post-close diff is not an array".into()))?;
    if rows.len() != request.limit().get() as usize {
        return Err(EastmoneyError::Protocol(format!(
            "post-close ranking returned {} rows for requested limit {}",
            rows.len(),
            request.limit().get()
        )));
    }

    let batch_id = format!(
        "eastmoney-web:post-close-flow:{}:{observed_at}",
        request.trading_date().as_str()
    );
    let mut instruments = HashSet::with_capacity(rows.len());
    let mut previous_main_net = None;
    let mut common_source_at: Option<String> = None;
    let mut mixed_source_times = false;
    let mut records = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let code = required_string(row, "f12")?;
        let market = optional_u32(row.get("f13"))?
            .ok_or_else(|| EastmoneyError::Protocol("post-close f13 is absent".into()))?;
        let instrument = instrument_from_market(&code, i64::from(market))?;
        if !instruments.insert(instrument.clone()) {
            return Err(EastmoneyError::Protocol(format!(
                "post-close ranking contains duplicate instrument {code}"
            )));
        }
        let main_net = required_f64(row.get("f62"), "f62")?;
        if previous_main_net.is_some_and(|previous| previous < main_net) {
            return Err(EastmoneyError::Protocol(
                "post-close f62 values are not in descending source order".into(),
            ));
        }
        previous_main_net = Some(main_net);
        let source_epoch = optional_u32(row.get("f124"))?
            .ok_or_else(|| EastmoneyError::Protocol("post-close f124 is absent".into()))?;
        let source_at = unix_seconds_to_china_rfc3339(i64::from(source_epoch))
            .map_err(|_| EastmoneyError::Protocol("post-close f124 is out of range".into()))?;
        if source_at.get(..10) != Some(request.trading_date().as_str()) {
            return Err(EastmoneyError::Protocol(format!(
                "post-close source timestamp {source_at} does not match requested date {}",
                request.trading_date().as_str()
            )));
        }
        match &common_source_at {
            Some(expected) if expected != &source_at => {
                if !allow_mixed_source_times {
                    return Err(EastmoneyError::Protocol(
                        "post-close source timestamps differ inside one ranking".into(),
                    ));
                }
                mixed_source_times = true;
            }
            None => common_source_at = Some(source_at.clone()),
            _ => {}
        }
        let evidence = SourceEvidence::new(
            ProviderId::Eastmoney,
            observed_at.to_owned(),
            batch_id.clone(),
        )?
        .with_source_at(source_at)?;
        let name = optional_nonempty(row.get("f14"))?
            .ok_or_else(|| EastmoneyError::Protocol("post-close f14 name is absent".into()))?;
        records.push(PostCloseFlow::new(
            instrument,
            Some(name),
            request.trading_date().clone(),
            PositiveU32::new(index as u32 + 1)?,
            Price::new(required_f64(row.get("f2"), "f2")?)?,
            Ratio::new(required_f64(row.get("f3"), "f3")?, RatioUnit::Percent)?,
            Money::new(main_net)?,
            Ratio::new(required_f64(row.get("f184"), "f184")?, RatioUnit::Percent)?,
            None,
            None,
            evidence,
        )?);
    }
    let source_at = common_source_at
        .ok_or_else(|| EastmoneyError::Protocol("post-close source time is absent".into()))?;
    let provenance = Provenance::new("eastmoney-web", observed_at)?.with_batch_id(batch_id)?;
    if mixed_source_times {
        Ok(DataBatch::best_effort(
            records,
            provenance,
            vec![
                "post-close records have mixed provider source times; snapshot is non-atomic"
                    .to_owned(),
            ],
        )?)
    } else {
        Ok(DataBatch::strict(
            records,
            provenance.with_source_at(source_at)?,
        ))
    }
}

fn validate_capture_window(
    request: &PostCloseFlowRequest,
    observed_at: &str,
) -> Result<(), EastmoneyError> {
    let prefix = format!("{}T", request.trading_date().as_str());
    let time = observed_at
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix("+08:00"))
        .ok_or_else(|| {
            EastmoneyError::InvalidRequest(
                "post-close ranking is available only for the current China trading date".into(),
            )
        })?;
    let time = ClockTime::parse(time)?;
    if time < ClockTime::parse("15:35:00")? {
        return Err(EastmoneyError::InvalidRequest(
            "post-close ranking cannot be captured before 15:35:00 Asia/Shanghai".into(),
        ));
    }
    Ok(())
}

fn validate_partial_capture_window(
    request: &PostCloseFlowRequest,
    observed_at: &str,
) -> Result<(), EastmoneyError> {
    let observed_date = observed_at
        .get(..10)
        .ok_or_else(|| EastmoneyError::InvalidRequest("post-close clock has no date".into()))?;
    let separator = observed_at.as_bytes().get(10).copied();
    let time = observed_at
        .get(11..19)
        .ok_or_else(|| EastmoneyError::InvalidRequest("post-close clock has no time".into()))?;
    let zone = observed_at.get(19..);
    let _ = magic_market_core::IsoDate::new(observed_date)?;
    let _ = ClockTime::parse(time)?;
    if separator != Some(b'T') || zone != Some("+08:00") {
        return Err(EastmoneyError::InvalidRequest(
            "post-close clock must be an Asia/Shanghai RFC3339 timestamp".into(),
        ));
    }
    match request.trading_date().as_str().cmp(observed_date) {
        std::cmp::Ordering::Greater => Err(EastmoneyError::InvalidRequest(
            "post-close diagnostic trading date cannot be in the future".into(),
        )),
        std::cmp::Ordering::Equal => validate_capture_window(request, observed_at),
        std::cmp::Ordering::Less => Ok(()),
    }
}

fn required_f64(value: Option<&Value>, field: &str) -> Result<f64, EastmoneyError> {
    optional_f64(value)?
        .ok_or_else(|| EastmoneyError::Protocol(format!("post-close {field} is absent")))
}

fn optional_nonempty(value: Option<&Value>) -> Result<Option<NonEmptyText>, EastmoneyError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() || matches!(value, "-" | "--") {
                Ok(None)
            } else {
                NonEmptyText::new(value).map(Some).map_err(Into::into)
            }
        }
        Some(other) => Err(EastmoneyError::Protocol(format!(
            "post-close name has invalid shape {other}"
        ))),
    }
}

pub(crate) fn china_now() -> Result<String, EastmoneyError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| EastmoneyError::Transport(format!("system clock error: {error}")))?
        .as_secs();
    let seconds = i64::try_from(seconds)
        .map_err(|_| EastmoneyError::Transport("system clock is outside i64".into()))?;
    unix_seconds_to_china_rfc3339(seconds)
        .map_err(|_| EastmoneyError::Transport("system clock is outside supported years".into()))
}

#[cfg(test)]
#[path = "../tests/internal/post_close_tests.rs"]
mod tests;
