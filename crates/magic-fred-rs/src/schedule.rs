use crate::FredError;
use magic_market_core::{
    DataBatch, EconomicReleaseScheduleEntry, EconomicReleaseScheduleRequest, IsoDate, PositiveU32,
    Provenance, ProviderId, SourceEvidence,
};
use magic_market_transport::{HttpTransport, RequestGate};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

const RELEASE_DATES_URL: &str = "https://api.stlouisfed.org/fred/releases/dates";
const PAGE_LIMIT: usize = 1000;
const MAX_PAGES: usize = 10;

#[derive(Deserialize)]
struct ScheduleEnvelope {
    realtime_start: String,
    realtime_end: String,
    order_by: String,
    sort_order: String,
    count: usize,
    offset: usize,
    limit: usize,
    release_dates: Vec<RawScheduleEntry>,
}

#[derive(Deserialize)]
struct RawScheduleEntry {
    release_id: u32,
    release_name: String,
    date: String,
    release_last_updated: Option<String>,
}

pub(crate) fn fetch_schedule(
    transport: &dyn HttpTransport,
    gate: &RequestGate,
    api_key: &str,
    request: &EconomicReleaseScheduleRequest,
) -> Result<DataBatch<EconomicReleaseScheduleEntry>, FredError> {
    let mut offset = 0_usize;
    let mut expected_count = None;
    let mut raw_entries = Vec::new();
    for _ in 0..MAX_PAGES {
        let offset_text = offset.to_string();
        let url = crate::transport::query_url(
            RELEASE_DATES_URL,
            &[
                ("api_key", api_key),
                ("file_type", "json"),
                ("realtime_start", request.start().as_str()),
                ("realtime_end", request.end().as_str()),
                ("limit", "1000"),
                ("offset", &offset_text),
                ("order_by", "release_date"),
                ("sort_order", "asc"),
                ("include_release_dates_with_no_data", "true"),
            ],
        );
        let body = crate::transport::execute(transport, gate, &url)?;
        let page = parse_schedule_page(&body, request, offset)?;
        if expected_count.is_some_and(|count| count != page.count) {
            return Err(FredError::Protocol(
                "release schedule count changed between pages".into(),
            ));
        }
        expected_count = Some(page.count);
        offset = offset
            .checked_add(page.entries.len())
            .ok_or_else(|| FredError::Protocol("release schedule offset overflow".into()))?;
        raw_entries.extend(page.entries);
        if offset == page.count {
            return normalize_schedule(raw_entries, request);
        }
    }
    Err(FredError::Protocol(
        "release schedule exceeds the ten-page provider limit".into(),
    ))
}

struct ParsedPage {
    count: usize,
    entries: Vec<RawScheduleEntry>,
}

fn parse_schedule_page(
    body: &[u8],
    request: &EconomicReleaseScheduleRequest,
    expected_offset: usize,
) -> Result<ParsedPage, FredError> {
    crate::transport::ensure_no_duplicate_json_keys(body)
        .map_err(|error| FredError::Decode(error.to_string()))?;
    let value: Value =
        serde_json::from_slice(body).map_err(|error| FredError::Decode(error.to_string()))?;
    if value.get("error_code").is_some() || value.get("error_message").is_some() {
        return Err(FredError::Authentication(
            "official API rejected the supplied credentials".into(),
        ));
    }
    let envelope: ScheduleEnvelope =
        serde_json::from_value(value).map_err(|error| FredError::Decode(error.to_string()))?;
    if envelope.realtime_start != request.start().as_str()
        || envelope.realtime_end != request.end().as_str()
        || envelope.order_by != "release_date"
        || envelope.sort_order != "asc"
        || envelope.offset != expected_offset
        || envelope.limit != PAGE_LIMIT
        || envelope.count > PAGE_LIMIT * MAX_PAGES
        || expected_offset > envelope.count
        || envelope.release_dates.len() > PAGE_LIMIT
        || envelope.release_dates.len() != (envelope.count - expected_offset).min(PAGE_LIMIT)
    {
        return Err(FredError::Protocol(
            "release schedule pagination or request identity is inconsistent".into(),
        ));
    }

    Ok(ParsedPage {
        count: envelope.count,
        entries: envelope.release_dates,
    })
}

fn normalize_schedule(
    raw_entries: Vec<RawScheduleEntry>,
    request: &EconomicReleaseScheduleRequest,
) -> Result<DataBatch<EconomicReleaseScheduleEntry>, FredError> {
    let observed_at = crate::transport::observed_at()?;
    let batch_id = format!("FRED:economic-release-schedule:{observed_at}");
    let mut records = Vec::with_capacity(raw_entries.len());
    let mut seen = HashSet::with_capacity(raw_entries.len());
    let mut previous_date: Option<IsoDate> = None;
    for raw in raw_entries {
        let release_id = PositiveU32::new(raw.release_id)?;
        let release_date = IsoDate::new(raw.date)?;
        if &release_date < request.start() || &release_date > request.end() {
            return Err(FredError::Protocol(
                "release date falls outside requested range".into(),
            ));
        }
        let identity = (raw.release_id, release_date.clone());
        if !seen.insert(identity.clone()) {
            return Err(FredError::Protocol(
                "release schedule contains duplicate identity".into(),
            ));
        }
        if previous_date
            .as_ref()
            .is_some_and(|value| value > &release_date)
        {
            return Err(FredError::Protocol(
                "release schedule dates are not ascending".into(),
            ));
        }
        previous_date = Some(release_date.clone());
        let evidence = SourceEvidence::new(ProviderId::Fred, &observed_at, &batch_id)?;
        records.push(EconomicReleaseScheduleEntry::new(
            release_id,
            raw.release_name,
            release_date,
            raw.release_last_updated,
            evidence,
        )?);
    }
    records.sort_by(|left, right| {
        left.release_date()
            .cmp(right.release_date())
            .then_with(|| left.release_id().cmp(&right.release_id()))
    });
    records.truncate(request.limit().get() as usize);
    Ok(DataBatch::strict(
        records,
        Provenance::new("FRED release schedule", observed_at)?.with_batch_id(batch_id)?,
    ))
}
