use crate::mapping::{optional_f64, optional_u32, required_string};
use crate::{instrument_from_market, query_url, EastmoneyClient, EastmoneyError};
use magic_market_core::{
    DataBatch, Money, NonEmptyText, PositiveU32, PostCloseFlow, PostCloseFlowRequest,
    PostCloseFlows, Price, Provenance, ProviderId, Ratio, RatioUnit, SourceEvidence,
};
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

    fn diagnose_post_close_flows_with_clock<Clock>(
        &self,
        request: &PostCloseFlowRequest,
        clock: Clock,
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
                        return parse_post_close(&bytes, request, &observed_at);
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

fn parse_post_close(
    bytes: &[u8],
    request: &PostCloseFlowRequest,
    observed_at: &str,
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
        let source_at = unix_seconds_to_china_iso(i64::from(source_epoch))
            .ok_or_else(|| EastmoneyError::Protocol("post-close f124 is out of range".into()))?;
        if source_at.get(..10) != Some(request.trading_date().as_str()) {
            return Err(EastmoneyError::Protocol(format!(
                "post-close source timestamp {source_at} does not match requested date {}",
                request.trading_date().as_str()
            )));
        }
        match &common_source_at {
            Some(expected) if expected != &source_at => {
                return Err(EastmoneyError::Protocol(
                    "post-close source timestamps differ inside one ranking".into(),
                ));
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
    let provenance = Provenance::new("eastmoney-web", observed_at)?
        .with_source_at(source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
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
    if time.len() != 8 || time < "15:35:00" {
        return Err(EastmoneyError::InvalidRequest(
            "post-close ranking cannot be captured before 15:35:00 Asia/Shanghai".into(),
        ));
    }
    Ok(())
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

fn china_now() -> Result<String, EastmoneyError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| EastmoneyError::Transport(format!("system clock error: {error}")))?
        .as_secs();
    let seconds = i64::try_from(seconds)
        .map_err(|_| EastmoneyError::Transport("system clock is outside i64".into()))?;
    unix_seconds_to_china_iso(seconds)
        .ok_or_else(|| EastmoneyError::Transport("system clock is outside supported years".into()))
}

pub(crate) fn unix_seconds_to_china_iso(seconds: i64) -> Option<String> {
    let local = seconds.checked_add(8 * 60 * 60)?;
    let days = local.div_euclid(86_400);
    let day_seconds = local.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days)?;
    let hour = day_seconds / 3_600;
    let minute = day_seconds % 3_600 / 60;
    let second = day_seconds % 60;
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}+08:00"
    ))
}

fn civil_from_days(days_since_epoch: i64) -> Option<(i64, i64, i64)> {
    let z = days_since_epoch.checked_add(719_468)?;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096).div_euclid(365);
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2).div_euclid(153);
    let day = doy - (153 * mp + 2).div_euclid(5) + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (1..=9999).contains(&year).then_some((year, month, day))
}

#[cfg(test)]
#[path = "../tests/internal/post_close_tests.rs"]
mod tests;
