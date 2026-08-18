use crate::mapping::{optional_f64, optional_string, optional_u32, required_string};
use crate::{instrument_from_market, query_url, BatchContext, EastmoneyClient, EastmoneyError};
use magic_market_core::{
    unix_seconds_to_china_rfc3339, validate_market_ranking_batch, Exchange, FiniteNumber,
    InstrumentId, IsoDate, MarketRankingEntry, MarketRankingKind, MarketRankingUnit,
    MarketRankings, MarketSession, NonEmptyText, PositiveU32, ProviderId, SourceEvidence,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;

const PRIMARY_ENDPOINT: &str = "https://push2.eastmoney.com/api/qt/clist/get";
const DELAY_ENDPOINT: &str = "https://push2delay.eastmoney.com/api/qt/clist/get";
const ENDPOINTS: [&str; 2] = [PRIMARY_ENDPOINT, DELAY_ENDPOINT];
const TOKEN: &str = "8dec03ba335b81bf4ebdf7b29ec27d15";
const A_SHARE_FILTER: &str =
    "m:0+t:6+f:!2,m:0+t:13+f:!2,m:0+t:80+f:!2,m:1+t:2+f:!2,m:1+t:23+f:!2,m:0+t:81+s:262144+f:!2";
const FIELDS: &str = "f1,f10,f12,f13,f14,f62,f124";
// The public endpoint currently caps `diff` at 100 even when `pz` is larger.
// Using the proved cap prevents false incomplete-page failures and keeps
// pagination offsets exact.
const PAGE_SIZE: u32 = 100;
const MAX_UNIVERSE_SIZE: u32 = 20_000;
const MAX_RETURNED_RANKS: u32 = 200;
const MAX_PAGE_ATTEMPTS: usize = 3;
const UNIVERSE: &str = "Eastmoney A-share equities";

/// A bounded first-page ranking is admitted only as one atomic provider
/// response. It does not claim a complete full-market pagination snapshot.
pub const BOUNDED_MARKET_RANKINGS_ADMITTED: bool = true;

/// One source-ranked row from a single bounded provider response. Optional
/// fields are retained for backward-compatible diagnostic decoding; the
/// admitted parser requires every identity, value and source-time field.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MarketRankingSnapshotEntry {
    kind: MarketRankingKind,
    source_rank: PositiveU32,
    instrument: Option<InstrumentId>,
    label: Option<String>,
    value: Option<FiniteNumber>,
    unit: MarketRankingUnit,
    source_at: Option<String>,
    reported_universe_size: PositiveU32,
    fetched_count: PositiveU32,
    evidence: SourceEvidence,
}

impl MarketRankingSnapshotEntry {
    pub fn evidence(&self) -> &SourceEvidence {
        &self.evidence
    }
}

pub type DiagnosticMarketRankingEntry = MarketRankingSnapshotEntry;

impl MarketRankings for EastmoneyClient {
    type Error = EastmoneyError;

    fn market_rankings(
        &self,
        kind: &MarketRankingKind,
        limit: PositiveU32,
    ) -> Result<magic_market_core::DataBatch<MarketRankingEntry>, Self::Error> {
        if limit.get() > MAX_RETURNED_RANKS {
            return Err(EastmoneyError::InvalidRequest(format!(
                "Eastmoney market ranking limit must be at most {MAX_RETURNED_RANKS}"
            )));
        }
        let field = ranking_field(kind)?;
        let mut last_transport_error = None;
        for endpoint in ENDPOINTS {
            match self.fetch_ranking_operation(endpoint, kind, field, limit) {
                Ok(batch) => return Ok(batch),
                Err(EastmoneyError::Transport(message)) => {
                    last_transport_error = Some(format!("{endpoint}: {message}"));
                }
                Err(error) => return Err(error),
            }
        }
        Err(EastmoneyError::Transport(format!(
            "all Eastmoney full-market HTTPS endpoints failed without a complete snapshot: {}",
            last_transport_error.unwrap_or_else(|| "no endpoint attempted".into())
        )))
    }
}

impl EastmoneyClient {
    /// Returns a bounded ranking snapshot from one HTTPS response. The source
    /// order, reported universe size, row count and per-row source time are
    /// preserved; no multi-page completeness claim is made.
    pub fn bounded_market_rankings_snapshot(
        &self,
        kind: &MarketRankingKind,
        limit: PositiveU32,
    ) -> Result<magic_market_core::DataBatch<MarketRankingSnapshotEntry>, EastmoneyError> {
        if limit.get() > PAGE_SIZE {
            return Err(EastmoneyError::InvalidRequest(format!(
                "Eastmoney bounded market ranking limit must be at most {PAGE_SIZE}"
            )));
        }
        let field = ranking_field(kind)?;
        let mut last_transport_error = None;
        for endpoint in ENDPOINTS {
            let url = ranking_url_for(endpoint, kind, field, 1, PAGE_SIZE)?;
            match self.fetch_ranking_page(&url) {
                Ok(bytes) => return parse_atomic_market_ranking_page(&bytes, kind, limit),
                Err(EastmoneyError::Transport(message)) => {
                    last_transport_error = Some(format!("{endpoint}: {message}"));
                }
                Err(error) => return Err(error),
            }
        }
        Err(EastmoneyError::Transport(format!(
            "all Eastmoney bounded ranking HTTPS endpoints failed: {}",
            last_transport_error.unwrap_or_else(|| "no endpoint attempted".into())
        )))
    }

    /// Fetches only the first bounded source ranking page for explicit
    /// diagnostic use. This method never claims complete-market coverage.
    pub fn diagnose_partial_market_rankings(
        &self,
        kind: &MarketRankingKind,
        limit: PositiveU32,
    ) -> Result<magic_market_core::DataBatch<DiagnosticMarketRankingEntry>, EastmoneyError> {
        if limit.get() > PAGE_SIZE {
            return Err(EastmoneyError::InvalidRequest(format!(
                "Eastmoney partial diagnostic ranking limit must be at most {PAGE_SIZE}"
            )));
        }
        let field = ranking_field(kind)?;
        let mut last_transport_error = None;
        for endpoint in ENDPOINTS {
            let url = ranking_url_for(endpoint, kind, field, 1, PAGE_SIZE)?;
            match self.fetch_ranking_page(&url) {
                Ok(bytes) => return parse_diagnostic_market_ranking_page(&bytes, kind, limit),
                Err(EastmoneyError::Transport(message)) => {
                    last_transport_error = Some(format!("{endpoint}: {message}"));
                }
                Err(error) => return Err(error),
            }
        }
        Err(EastmoneyError::Transport(format!(
            "all Eastmoney diagnostic ranking HTTPS endpoints failed: {}",
            last_transport_error.unwrap_or_else(|| "no endpoint attempted".into())
        )))
    }

    fn fetch_ranking_operation(
        &self,
        endpoint: &str,
        kind: &MarketRankingKind,
        field: &str,
        limit: PositiveU32,
    ) -> Result<magic_market_core::DataBatch<MarketRankingEntry>, EastmoneyError> {
        let mut pages = Vec::new();
        let mut page = 1_u32;
        let mut expected_total = None;
        loop {
            let url = ranking_url_for(endpoint, kind, field, page, PAGE_SIZE)?;
            let bytes = self.fetch_ranking_page(&url)?;
            let envelope = parse_page(&bytes)?;
            if envelope.total == 0 {
                return Err(EastmoneyError::Protocol(
                    "Eastmoney full-market ranking returned an empty universe".into(),
                ));
            }
            if envelope.total > MAX_UNIVERSE_SIZE {
                return Err(EastmoneyError::Protocol(format!(
                    "Eastmoney ranking universe {} exceeds safety limit {MAX_UNIVERSE_SIZE}",
                    envelope.total
                )));
            }
            match expected_total {
                Some(total) if total != envelope.total => {
                    return Err(EastmoneyError::Protocol(format!(
                        "Eastmoney ranking total changed across pages: {total} to {}",
                        envelope.total
                    )))
                }
                None => expected_total = Some(envelope.total),
                _ => {}
            }
            let received = u32::try_from(envelope.rows.len()).map_err(|_| {
                EastmoneyError::Protocol("Eastmoney ranking page length overflow".into())
            })?;
            let page_total = envelope.total;
            let consumed = page
                .checked_sub(1)
                .and_then(|value| value.checked_mul(PAGE_SIZE))
                .and_then(|value| value.checked_add(received))
                .ok_or_else(|| EastmoneyError::Protocol("ranking pagination overflow".into()))?;
            pages.push(envelope);
            if consumed >= page_total {
                break;
            }
            if received != PAGE_SIZE {
                return Err(EastmoneyError::Protocol(format!(
                    "Eastmoney ranking page {page} returned {received} rows before total {}",
                    page_total
                )));
            }
            page = page
                .checked_add(1)
                .ok_or_else(|| EastmoneyError::Protocol("ranking page overflow".into()))?;
        }
        parse_market_ranking_envelopes(pages, kind, limit, PAGE_SIZE)
    }

    fn fetch_ranking_page(&self, url: &str) -> Result<Vec<u8>, EastmoneyError> {
        for attempt in 1..=MAX_PAGE_ATTEMPTS {
            let result = self.get(
                url,
                &[
                    ("Accept", "application/json"),
                    ("Referer", "https://data.eastmoney.com/"),
                ],
            );
            match result {
                Err(EastmoneyError::Transport(_)) if attempt < MAX_PAGE_ATTEMPTS => continue,
                result => return result,
            }
        }
        Err(EastmoneyError::Transport(
            "market ranking retry loop exhausted without a terminal result".into(),
        ))
    }
}

fn ranking_field(kind: &MarketRankingKind) -> Result<&'static str, EastmoneyError> {
    match kind {
        MarketRankingKind::VolumeRatio => Ok("f10"),
        MarketRankingKind::MainNetInflow => Ok("f62"),
        other => Err(EastmoneyError::Unsupported(format!(
            "Eastmoney full-market ranking kind {other:?} is not source-proven"
        ))),
    }
}

fn ranking_unit(kind: &MarketRankingKind) -> Result<MarketRankingUnit, EastmoneyError> {
    match kind {
        MarketRankingKind::VolumeRatio => Ok(MarketRankingUnit::Multiple),
        MarketRankingKind::MainNetInflow => Ok(MarketRankingUnit::Yuan),
        other => Err(EastmoneyError::Unsupported(format!(
            "Eastmoney full-market ranking kind {other:?} is not source-proven"
        ))),
    }
}

#[cfg(test)]
fn ranking_url(
    kind: &MarketRankingKind,
    field: &str,
    page: u32,
    page_size: u32,
) -> Result<String, EastmoneyError> {
    ranking_url_for(PRIMARY_ENDPOINT, kind, field, page, page_size)
}

fn ranking_url_for(
    endpoint: &str,
    kind: &MarketRankingKind,
    field: &str,
    page: u32,
    page_size: u32,
) -> Result<String, EastmoneyError> {
    if !ENDPOINTS.contains(&endpoint) {
        return Err(EastmoneyError::InvalidRequest(
            "unregistered Eastmoney ranking endpoint".into(),
        ));
    }
    if ranking_field(kind)? != field || page == 0 || page_size == 0 {
        return Err(EastmoneyError::InvalidRequest(
            "invalid Eastmoney ranking pagination or field".into(),
        ));
    }
    Ok(query_url(
        endpoint,
        &[
            ("pn", page.to_string()),
            ("pz", page_size.to_string()),
            ("po", "1".into()),
            ("np", "1".into()),
            ("ut", TOKEN.into()),
            ("fltt", "2".into()),
            ("invt", "2".into()),
            ("fid", field.into()),
            ("fs", A_SHARE_FILTER.into()),
            ("fields", FIELDS.into()),
        ],
    ))
}

struct PageEnvelope {
    total: u32,
    rows: Vec<Value>,
}

fn parse_page(bytes: &[u8]) -> Result<PageEnvelope, EastmoneyError> {
    let root: Value =
        serde_json::from_slice(bytes).map_err(|error| EastmoneyError::Decode(error.to_string()))?;
    if root.get("rc").and_then(Value::as_i64) != Some(0) {
        return Err(EastmoneyError::Protocol(format!(
            "Eastmoney market ranking returned rc {:?}",
            root.get("rc")
        )));
    }
    let data = root
        .get("data")
        .and_then(Value::as_object)
        .ok_or_else(|| EastmoneyError::Protocol("market ranking data is absent".into()))?;
    let total = optional_u32(data.get("total"))?
        .ok_or_else(|| EastmoneyError::Protocol("market ranking total is absent".into()))?;
    let rows = data
        .get("diff")
        .and_then(Value::as_array)
        .ok_or_else(|| EastmoneyError::Protocol("market ranking diff is not an array".into()))?
        .clone();
    Ok(PageEnvelope { total, rows })
}

fn parse_diagnostic_market_ranking_page(
    bytes: &[u8],
    kind: &MarketRankingKind,
    limit: PositiveU32,
) -> Result<magic_market_core::DataBatch<DiagnosticMarketRankingEntry>, EastmoneyError> {
    if limit.get() > PAGE_SIZE {
        return Err(EastmoneyError::InvalidRequest(format!(
            "Eastmoney partial diagnostic ranking limit must be at most {PAGE_SIZE}"
        )));
    }
    let envelope = parse_page(bytes)?;
    if envelope.total == 0 || envelope.total > MAX_UNIVERSE_SIZE {
        return Err(EastmoneyError::Protocol(format!(
            "Eastmoney diagnostic ranking universe {} is outside 1..={MAX_UNIVERSE_SIZE}",
            envelope.total
        )));
    }
    let expected_page_count = envelope.total.min(PAGE_SIZE);
    let fetched_count = u32::try_from(envelope.rows.len())
        .map_err(|_| EastmoneyError::Protocol("ranking page length overflow".into()))?;
    if fetched_count != expected_page_count {
        return Err(EastmoneyError::Protocol(format!(
            "Eastmoney diagnostic ranking first page returned {fetched_count} rows; expected {expected_page_count}"
        )));
    }

    let field = ranking_field(kind)?;
    let unit = ranking_unit(kind)?;
    let context = BatchContext::new("market-ranking-diagnostic", None)?;
    let returned = usize::try_from(limit.get().min(fetched_count))
        .map_err(|_| EastmoneyError::Protocol("ranking limit overflow".into()))?;
    let mut missing_fields = 0_u32;
    let mut records = Vec::with_capacity(returned);
    for (index, row) in envelope.rows.into_iter().take(returned).enumerate() {
        let code = optional_string(row.get("f12"))?;
        let market = optional_u32(row.get("f13"))?;
        let instrument = match (code.as_deref(), market) {
            (Some(code), Some(market)) => Some(instrument_from_market(code, i64::from(market))?),
            _ => {
                missing_fields = missing_fields.saturating_add(1);
                None
            }
        };
        let label = optional_string(row.get("f14"))?;
        if label.is_none() {
            missing_fields = missing_fields.saturating_add(1);
        }
        let value = optional_f64(row.get(field))?
            .map(FiniteNumber::new)
            .transpose()?;
        if value.is_none() {
            missing_fields = missing_fields.saturating_add(1);
        }
        if matches!(kind, MarketRankingKind::VolumeRatio)
            && value.is_some_and(|value| value.get().is_sign_negative())
        {
            return Err(EastmoneyError::Protocol(
                "market ranking volume ratio must be non-negative".into(),
            ));
        }
        let source_at = optional_u32(row.get("f124"))?
            .filter(|epoch| *epoch > 0)
            .map(|epoch| {
                unix_seconds_to_china_rfc3339(i64::from(epoch)).map_err(|_| {
                    EastmoneyError::Protocol("market ranking f124 is out of range".into())
                })
            })
            .transpose()?;
        if source_at.is_none() {
            missing_fields = missing_fields.saturating_add(1);
        }
        records.push(DiagnosticMarketRankingEntry {
            kind: kind.clone(),
            source_rank: PositiveU32::new(
                u32::try_from(index + 1)
                    .map_err(|_| EastmoneyError::Protocol("ranking rank overflow".into()))?,
            )?,
            instrument,
            label,
            value,
            unit: unit.clone(),
            source_at: source_at.clone(),
            reported_universe_size: PositiveU32::new(envelope.total)?,
            fetched_count: PositiveU32::new(fetched_count)?,
            evidence: context.evidence_at(source_at.as_deref())?,
        });
    }
    let mut issues = vec![format!(
        "diagnostic fetched the first {fetched_count} of {} source-ranked rows; complete-market coverage is not claimed",
        envelope.total
    )];
    if missing_fields > 0 {
        issues.push(format!(
            "{missing_fields} optional fields were absent in returned rows and remain null"
        ));
    }
    context.finish_with_issues(records, issues)
}

fn parse_atomic_market_ranking_page(
    bytes: &[u8],
    kind: &MarketRankingKind,
    limit: PositiveU32,
) -> Result<magic_market_core::DataBatch<MarketRankingSnapshotEntry>, EastmoneyError> {
    if limit.get() > PAGE_SIZE {
        return Err(EastmoneyError::InvalidRequest(format!(
            "Eastmoney bounded market ranking limit must be at most {PAGE_SIZE}"
        )));
    }
    let envelope = parse_page(bytes)?;
    if envelope.total == 0 || envelope.total > MAX_UNIVERSE_SIZE {
        return Err(EastmoneyError::Protocol(format!(
            "Eastmoney ranking universe {} is outside 1..={MAX_UNIVERSE_SIZE}",
            envelope.total
        )));
    }
    let expected_page_count = envelope.total.min(PAGE_SIZE);
    let fetched_count = u32::try_from(envelope.rows.len())
        .map_err(|_| EastmoneyError::Protocol("ranking page length overflow".into()))?;
    if fetched_count != expected_page_count {
        return Err(EastmoneyError::Protocol(format!(
            "Eastmoney bounded ranking response returned {fetched_count} rows; expected {expected_page_count}"
        )));
    }

    let field = ranking_field(kind)?;
    let unit = ranking_unit(kind)?;
    let context = BatchContext::new("market-ranking-snapshot", None)?;
    let returned = usize::try_from(limit.get().min(fetched_count))
        .map_err(|_| EastmoneyError::Protocol("ranking limit overflow".into()))?;
    let mut seen = HashSet::with_capacity(returned);
    let mut previous_value = None;
    let mut records = Vec::with_capacity(returned);
    for (index, row) in envelope.rows.into_iter().take(returned).enumerate() {
        let code = optional_string(row.get("f12"))?
            .ok_or_else(|| EastmoneyError::Protocol("market ranking f12 is absent".into()))?;
        let market = optional_u32(row.get("f13"))?
            .ok_or_else(|| EastmoneyError::Protocol("market ranking f13 is absent".into()))?;
        let instrument = instrument_from_market(&code, i64::from(market))?;
        if !seen.insert(instrument.clone()) {
            return Err(EastmoneyError::Protocol(format!(
                "market ranking contains duplicate instrument {instrument:?}"
            )));
        }
        let label = optional_string(row.get("f14"))?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| EastmoneyError::Protocol("market ranking f14 is absent".into()))?;
        let value = FiniteNumber::new(optional_f64(row.get(field))?.ok_or_else(|| {
            EastmoneyError::Protocol(format!("market ranking {field} is absent"))
        })?)?;
        if matches!(kind, MarketRankingKind::VolumeRatio) && value.get().is_sign_negative() {
            return Err(EastmoneyError::Protocol(
                "market ranking volume ratio must be non-negative".into(),
            ));
        }
        if previous_value.is_some_and(|previous: f64| previous < value.get()) {
            return Err(EastmoneyError::Protocol(
                "market ranking source order is not descending".into(),
            ));
        }
        previous_value = Some(value.get());
        let epoch = optional_u32(row.get("f124"))?
            .filter(|epoch| *epoch > 0)
            .ok_or_else(|| EastmoneyError::Protocol("market ranking f124 is absent".into()))?;
        let source_at = unix_seconds_to_china_rfc3339(i64::from(epoch))
            .map_err(|_| EastmoneyError::Protocol("market ranking f124 is out of range".into()))?;
        records.push(MarketRankingSnapshotEntry {
            kind: kind.clone(),
            source_rank: PositiveU32::new(
                u32::try_from(index + 1)
                    .map_err(|_| EastmoneyError::Protocol("ranking rank overflow".into()))?,
            )?,
            instrument: Some(instrument),
            label: Some(label),
            value: Some(value),
            unit: unit.clone(),
            source_at: Some(source_at.clone()),
            reported_universe_size: PositiveU32::new(envelope.total)?,
            fetched_count: PositiveU32::new(fetched_count)?,
            evidence: context.evidence_at(Some(&source_at))?,
        });
    }
    context.finish(records)
}

#[cfg(test)]
fn parse_market_ranking_pages(
    pages: &[Vec<u8>],
    kind: &MarketRankingKind,
    limit: PositiveU32,
    page_size: u32,
) -> Result<magic_market_core::DataBatch<MarketRankingEntry>, EastmoneyError> {
    let pages = pages
        .iter()
        .map(|bytes| parse_page(bytes))
        .collect::<Result<Vec<_>, _>>()?;
    parse_market_ranking_envelopes(pages, kind, limit, page_size)
}

fn parse_market_ranking_envelopes(
    pages: Vec<PageEnvelope>,
    kind: &MarketRankingKind,
    limit: PositiveU32,
    page_size: u32,
) -> Result<magic_market_core::DataBatch<MarketRankingEntry>, EastmoneyError> {
    if pages.is_empty() || page_size == 0 {
        return Err(EastmoneyError::InvalidRequest(
            "market ranking requires at least one bounded source page".into(),
        ));
    }
    let field = ranking_field(kind)?;
    let unit = ranking_unit(kind)?;
    let mut total = None;
    let mut rows = Vec::<Value>::new();
    for (index, envelope) in pages.into_iter().enumerate() {
        match total {
            Some(expected) if expected != envelope.total => {
                return Err(EastmoneyError::Protocol(format!(
                    "market ranking total changed from {expected} to {} on page {}",
                    envelope.total,
                    index + 1
                )))
            }
            None => total = Some(envelope.total),
            _ => {}
        }
        if envelope.rows.len() > page_size as usize {
            return Err(EastmoneyError::Protocol(format!(
                "market ranking page {} exceeds declared page size {page_size}",
                index + 1
            )));
        }
        rows.extend(envelope.rows);
    }
    let total =
        total.ok_or_else(|| EastmoneyError::Protocol("market ranking total is absent".into()))?;
    if total == 0 || total > MAX_UNIVERSE_SIZE || rows.len() != total as usize {
        return Err(EastmoneyError::Protocol(format!(
            "market ranking pagination collected {} rows for declared total {total}",
            rows.len()
        )));
    }

    let mut mapped = Vec::with_capacity(rows.len());
    let mut instruments = HashSet::with_capacity(rows.len());
    let mut exchanges = HashSet::with_capacity(3);
    let mut previous = None;
    let mut earliest = None::<i64>;
    let mut latest = None::<i64>;
    let mut source_date = None::<IsoDate>;
    let mut session = None::<MarketSession>;
    for row in &rows {
        let code = required_string(row, "f12")?;
        let market = optional_u32(row.get("f13"))?
            .ok_or_else(|| EastmoneyError::Protocol("market ranking f13 is absent".into()))?;
        let instrument = instrument_from_market(&code, i64::from(market))?;
        exchanges.insert(instrument.exchange());
        if !instruments.insert(instrument.clone()) {
            return Err(EastmoneyError::Protocol(format!(
                "market ranking contains duplicate instrument {code}"
            )));
        }
        let name = NonEmptyText::new(required_string(row, "f14")?)?;
        let value = optional_f64(row.get(field))?
            .ok_or_else(|| EastmoneyError::Protocol(format!("market ranking {field} is absent")))?;
        if matches!(kind, MarketRankingKind::VolumeRatio) && value.is_sign_negative() {
            return Err(EastmoneyError::Protocol(
                "market ranking volume ratio must be non-negative".into(),
            ));
        }
        if previous.is_some_and(|previous| previous < value) {
            return Err(EastmoneyError::Protocol(format!(
                "market ranking {field} is not in descending source order"
            )));
        }
        previous = Some(value);
        let epoch = optional_u32(row.get("f124"))?
            .filter(|epoch| *epoch > 0)
            .ok_or_else(|| EastmoneyError::Protocol("market ranking f124 is absent".into()))?;
        let source_at = unix_seconds_to_china_rfc3339(i64::from(epoch))
            .map_err(|_| EastmoneyError::Protocol("market ranking f124 is out of range".into()))?;
        let date = IsoDate::new(
            source_at
                .get(..10)
                .ok_or_else(|| EastmoneyError::Protocol("source time has no date".into()))?,
        )?;
        let row_session = session_for_source_at(&source_at)?;
        match &source_date {
            Some(expected) if expected != &date => {
                return Err(EastmoneyError::Protocol(
                    "market ranking source dates differ across the universe".into(),
                ))
            }
            None => source_date = Some(date),
            _ => {}
        }
        match session {
            Some(expected) if expected != row_session => {
                return Err(EastmoneyError::Protocol(
                    "market ranking source sessions differ across the universe".into(),
                ))
            }
            None => session = Some(row_session),
            _ => {}
        }
        earliest = Some(earliest.map_or(i64::from(epoch), |value| value.min(i64::from(epoch))));
        latest = Some(latest.map_or(i64::from(epoch), |value| value.max(i64::from(epoch))));
        mapped.push((instrument, name, value, source_at));
    }
    let required_exchanges =
        HashSet::from([Exchange::Shanghai, Exchange::Shenzhen, Exchange::Beijing]);
    if exchanges != required_exchanges {
        return Err(EastmoneyError::Protocol(format!(
            "full-market ranking exchange coverage is incomplete: {exchanges:?}"
        )));
    }
    let source_date =
        source_date.ok_or_else(|| EastmoneyError::Protocol("ranking source date absent".into()))?;
    let session =
        session.ok_or_else(|| EastmoneyError::Protocol("ranking source session absent".into()))?;
    let skew = latest
        .and_then(|latest| earliest.map(|earliest| latest - earliest))
        .and_then(|seconds| u64::try_from(seconds).ok())
        .and_then(|seconds| seconds.checked_mul(1_000))
        .ok_or_else(|| EastmoneyError::Protocol("market ranking source skew overflow".into()))?;
    if skew != 0 {
        return Err(EastmoneyError::Protocol(format!(
            "market ranking rows do not share one source time; skew is {skew}ms"
        )));
    }
    let common_source_at = mapped
        .iter()
        .min_by_key(|(_, _, _, source_at)| source_at.as_str())
        .map(|(_, _, _, source_at)| source_at.as_str())
        .ok_or_else(|| EastmoneyError::Protocol("ranking source time absent".into()))?;
    let context = BatchContext::new("market-ranking", Some(common_source_at))?;
    let returned = usize::try_from(limit.get().min(total))
        .map_err(|_| EastmoneyError::Protocol("ranking limit overflow".into()))?;
    let mut records = Vec::with_capacity(returned);
    for (index, (instrument, name, value, source_at)) in
        mapped.into_iter().take(returned).enumerate()
    {
        records.push(MarketRankingEntry::new(
            kind.clone(),
            PositiveU32::new(
                u32::try_from(index + 1)
                    .map_err(|_| EastmoneyError::Protocol("ranking rank overflow".into()))?,
            )?,
            Some(instrument),
            name,
            FiniteNumber::new(value)?,
            unit.clone(),
            source_date.clone(),
            session,
            NonEmptyText::new(UNIVERSE)?,
            PositiveU32::new(total)?,
            PositiveU32::new(total)?,
            skew,
            context.evidence_at(Some(&source_at))?,
        )?);
    }
    validate_market_ranking_batch(&records, kind, limit)?;
    let batch = context.finish(records)?;
    if batch
        .records()
        .iter()
        .any(|record| record.evidence().provider() != ProviderId::Eastmoney)
    {
        return Err(EastmoneyError::Protocol(
            "market ranking evidence provider mismatch".into(),
        ));
    }
    Ok(batch)
}

fn session_for_source_at(source_at: &str) -> Result<MarketSession, EastmoneyError> {
    let time = source_at.get(11..19).ok_or_else(|| {
        EastmoneyError::Protocol(format!("source time {source_at:?} has no clock"))
    })?;
    if time < "09:15:00" {
        Ok(MarketSession::PreOpen)
    } else if time < "09:30:00" {
        Ok(MarketSession::OpeningAuction)
    } else if time <= "11:30:00" {
        Ok(MarketSession::Continuous)
    } else if time < "13:00:00" {
        Ok(MarketSession::LunchBreak)
    } else if time < "15:00:00" {
        Ok(MarketSession::Continuous)
    } else if time == "15:00:00" {
        Ok(MarketSession::Close)
    } else {
        Ok(MarketSession::PostClose)
    }
}

#[cfg(test)]
#[path = "../tests/internal/market_rankings_tests.rs"]
mod tests;
