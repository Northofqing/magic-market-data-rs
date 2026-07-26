use super::{
    announcement_url, encode_form, ensure_json, form_headers, normalize_required, now,
    optional_nonempty, parse_required_millis, pdf_url, provenance, required_text, CninfoClient,
    CninfoError, HttpMethod, HttpRequest, PAGE_SIZE,
};
use magic_market_core::{
    Announcement, AssetClass, DataBatch, Exchange, InstrumentId, MarketAnnouncementRequest,
    MarketAnnouncements, NonEmptyText, ProviderId, SourceEvidence,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

const MARKET_COLUMN: &str = "szse";

#[derive(Debug, Deserialize)]
struct MarketAnnouncementPage {
    #[serde(rename = "totalAnnouncement")]
    total_announcement: Option<u64>,
    #[serde(rename = "totalRecordNum")]
    total_record_num: Option<u64>,
    #[serde(rename = "totalpages")]
    total_pages: Option<u64>,
    #[serde(rename = "hasMore")]
    has_more: Option<bool>,
    announcements: Option<Vec<MarketAnnouncementWire>>,
}

#[derive(Debug, Deserialize)]
struct MarketAnnouncementWire {
    #[serde(rename = "announcementId")]
    announcement_id: Option<String>,
    #[serde(rename = "secCode")]
    security_code: Option<String>,
    #[serde(rename = "secName")]
    security_name: Option<String>,
    #[serde(rename = "orgId")]
    organization_id: Option<String>,
    #[serde(rename = "announcementTitle")]
    title: Option<String>,
    #[serde(rename = "announcementTypeName")]
    category_name: Option<String>,
    #[serde(rename = "announcementType")]
    category: Option<String>,
    #[serde(rename = "announcementTime")]
    published_at: Option<Value>,
    #[serde(rename = "adjunctUrl")]
    adjunct_url: Option<String>,
    #[serde(rename = "pageColumn")]
    page_column: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidatedAnnouncement {
    announcement_id: String,
    security_code: String,
    security_name: Option<String>,
    organization_id: String,
    exchange: Exchange,
    title: String,
    category: Option<String>,
    published_at: String,
    adjunct_url: Option<String>,
    page_column: String,
}

impl CninfoClient {
    fn market_announcement_page(
        &self,
        request: &MarketAnnouncementRequest,
        page: u32,
    ) -> Result<MarketAnnouncementPage, CninfoError> {
        let body = encode_form(&[
            ("stock", String::new()),
            ("tabName", "fulltext".into()),
            ("pageSize", PAGE_SIZE.to_string()),
            ("pageNum", page.to_string()),
            ("column", MARKET_COLUMN.into()),
            ("category", String::new()),
            ("plate", String::new()),
            (
                "seDate",
                format!("{}~{}", request.start().as_str(), request.end().as_str()),
            ),
            ("searchkey", String::new()),
            ("secid", String::new()),
            ("sortName", String::new()),
            ("sortType", String::new()),
            ("isHLtitle", "false".into()),
        ]);
        let response = self.execute(HttpRequest {
            method: HttpMethod::Post,
            url: self.config.announcement_url.clone(),
            headers: form_headers(
                "https://www.cninfo.com.cn",
                "https://www.cninfo.com.cn/new/disclosure",
            ),
            body,
        })?;
        ensure_json(&response)?;
        serde_json::from_slice(&response.body)
            .map_err(|error| CninfoError::Decode(error.to_string()))
    }
}

impl MarketAnnouncements for CninfoClient {
    type Error = CninfoError;

    fn market_announcements(
        &self,
        request: &MarketAnnouncementRequest,
    ) -> Result<DataBatch<Announcement>, Self::Error> {
        let limit = request.limit().get() as usize;
        let mut expected_total = None;
        let mut consumed_rows = 0_u64;
        let mut pages_read = 0_u32;
        let mut previous_source_at: Option<String> = None;
        let mut seen = HashMap::<String, ValidatedAnnouncement>::new();
        let mut validated = Vec::with_capacity(limit);

        while validated.len() < limit {
            let page = pages_read.checked_add(1).ok_or_else(|| {
                CninfoError::Incomplete("market announcement page overflow".into())
            })?;
            if page > self.config.max_pages {
                return Err(CninfoError::Incomplete(format!(
                    "market announcement limit {limit} requires more than {} complete pages",
                    self.config.max_pages
                )));
            }
            let document = self.market_announcement_page(request, page)?;
            let (total, rows) =
                validate_market_page(document, page, expected_total, consumed_rows)?;
            expected_total = Some(total);
            pages_read = page;

            if total == 0 {
                let observed_at = now()?;
                let batch_id = format!(
                    "cninfo:{observed_at}:market-announcements:{}:{}:pages=1:total=0",
                    request.start().as_str(),
                    request.end().as_str()
                );
                let provenance = provenance("cninfo-market", &observed_at, &batch_id, None)?;
                return Ok(DataBatch::strict(Vec::new(), provenance));
            }

            consumed_rows = consumed_rows
                .checked_add(rows.len() as u64)
                .ok_or_else(|| {
                    CninfoError::Incomplete("market announcement row count overflow".into())
                })?;
            for row in rows {
                let row = validate_market_row(row, request)?;
                if previous_source_at
                    .as_deref()
                    .is_some_and(|previous| row.published_at.as_str() > previous)
                {
                    return Err(CninfoError::Incomplete(
                        "market announcement source order changed within the fetched prefix".into(),
                    ));
                }
                previous_source_at = Some(row.published_at.clone());
                match seen.get(&row.announcement_id) {
                    Some(existing) if existing == &row => {}
                    Some(_) => {
                        return Err(CninfoError::Schema(format!(
                            "conflicting market announcement {} across pages",
                            row.announcement_id
                        )));
                    }
                    None => {
                        seen.insert(row.announcement_id.clone(), row.clone());
                        validated.push(row);
                    }
                }
            }

            if consumed_rows >= total {
                break;
            }
        }

        let total = expected_total.ok_or_else(|| {
            CninfoError::Incomplete("market announcement source total is unavailable".into())
        })?;
        let observed_at = now()?;
        let batch_id = format!(
            "cninfo:{observed_at}:market-announcements:{}:{}:pages={pages_read}:total={total}",
            request.start().as_str(),
            request.end().as_str()
        );
        validated.truncate(limit);
        let records = validated
            .into_iter()
            .map(|row| map_market_row(row, &observed_at, &batch_id))
            .collect::<Result<Vec<_>, _>>()?;
        let source_at = records.first().map(|record| record.published_at.as_str());
        let provenance = provenance("cninfo-market", &observed_at, &batch_id, source_at)?;
        Ok(DataBatch::strict(records, provenance))
    }
}

fn validate_market_page(
    document: MarketAnnouncementPage,
    page: u32,
    expected_total: Option<u64>,
    consumed_rows: u64,
) -> Result<(u64, Vec<MarketAnnouncementWire>), CninfoError> {
    let total = document
        .total_announcement
        .ok_or_else(|| CninfoError::Schema("market totalAnnouncement is missing".into()))?;
    let total_records = document
        .total_record_num
        .ok_or_else(|| CninfoError::Schema("market totalRecordNum is missing".into()))?;
    if total != total_records {
        return Err(CninfoError::Incomplete(format!(
            "market totals disagree: totalAnnouncement={total} totalRecordNum={total_records}"
        )));
    }
    if expected_total.is_some_and(|expected| expected != total) {
        return Err(CninfoError::Incomplete(
            "market total changed between pages".into(),
        ));
    }
    let total_pages = document
        .total_pages
        .ok_or_else(|| CninfoError::Schema("market totalpages is missing".into()))?;
    // CNInfo reports the integer quotient, not a conventional one-based page
    // count. For example, total=1108 with pageSize=30 reports totalpages=36
    // while the complete result spans 37 request pages.
    let expected_source_pages = total / u64::from(PAGE_SIZE);
    if total_pages != expected_source_pages {
        return Err(CninfoError::Incomplete(format!(
            "market totalpages={total_pages} does not match total {total}"
        )));
    }
    let rows = document.announcements.unwrap_or_default();
    if total == 0 {
        let has_more = document
            .has_more
            .ok_or_else(|| CninfoError::Schema("market hasMore is missing".into()))?;
        if page != 1 || consumed_rows != 0 || has_more || !rows.is_empty() {
            return Err(CninfoError::Incomplete(
                "market zero-total response has rows or an invalid page boundary".into(),
            ));
        }
        return Ok((0, rows));
    }
    let actual_page_count = total.div_ceil(u64::from(PAGE_SIZE));
    if u64::from(page) > actual_page_count || consumed_rows >= total {
        return Err(CninfoError::Incomplete(
            "market returned a page beyond the declared total".into(),
        ));
    }
    let expected_rows = (total - consumed_rows).min(u64::from(PAGE_SIZE)) as usize;
    if rows.len() != expected_rows {
        return Err(CninfoError::Incomplete(format!(
            "market page {page} has {} rows, expected {expected_rows}",
            rows.len()
        )));
    }
    let has_more = document
        .has_more
        .ok_or_else(|| CninfoError::Schema("market hasMore is missing".into()))?;
    let expected_has_more = consumed_rows + (expected_rows as u64) < total;
    if has_more != expected_has_more {
        return Err(CninfoError::Incomplete(format!(
            "market page {page} hasMore={has_more} expected {expected_has_more}"
        )));
    }
    Ok((total, rows))
}

fn validate_market_row(
    row: MarketAnnouncementWire,
    request: &MarketAnnouncementRequest,
) -> Result<ValidatedAnnouncement, CninfoError> {
    let security_code = required_text(row.security_code, "market announcement.secCode")?;
    if security_code.len() != 6 || !security_code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(CninfoError::Schema(format!(
            "market announcement secCode {security_code:?} is not six ASCII digits"
        )));
    }
    let page_column = required_text(row.page_column, "market announcement.pageColumn")?;
    let exchange = match page_column.as_str() {
        "SHMB" | "SHZB" | "SHKCP" | "SHKCB" => Exchange::Shanghai,
        "SZMB" | "SZZB" | "SZCY" => Exchange::Shenzhen,
        "BJS" => Exchange::Beijing,
        value => {
            return Err(CninfoError::Unsupported(format!(
                "market announcement pageColumn {value:?} is not a verified A-share equity board"
            )));
        }
    };
    let published_at = parse_required_millis(row.published_at.as_ref(), "market announcementTime")?;
    let source_date = published_at
        .get(..10)
        .ok_or_else(|| CninfoError::Schema("market source timestamp has no date".into()))?;
    if source_date < request.start().as_str() || source_date > request.end().as_str() {
        return Err(CninfoError::Schema(format!(
            "market announcement date {source_date} is outside the requested range"
        )));
    }
    Ok(ValidatedAnnouncement {
        announcement_id: required_text(row.announcement_id, "market announcement.announcementId")?,
        security_code,
        security_name: row
            .security_name
            .map(super::normalize_text)
            .and_then(super::nonblank),
        organization_id: required_text(row.organization_id, "market announcement.orgId")?,
        exchange,
        title: normalize_required(row.title, "market announcement.announcementTitle")?,
        category: row
            .category_name
            .or(row.category)
            .map(super::normalize_text)
            .and_then(super::nonblank),
        published_at,
        adjunct_url: row.adjunct_url.and_then(super::nonblank),
        page_column,
    })
}

fn map_market_row(
    row: ValidatedAnnouncement,
    observed_at: &str,
    batch_id: &str,
) -> Result<Announcement, CninfoError> {
    let instrument = InstrumentId::new(row.exchange, &row.security_code, AssetClass::Equity)?;
    let mut evidence = SourceEvidence::new(ProviderId::Cninfo, observed_at, batch_id)?;
    evidence = evidence.with_source_at(&row.published_at)?;
    Ok(Announcement {
        announcement_id: NonEmptyText::new(row.announcement_id.clone())?,
        instrument,
        instrument_name: optional_nonempty(row.security_name)?,
        category: optional_nonempty(row.category)?,
        title: NonEmptyText::new(row.title)?,
        published_at: NonEmptyText::new(row.published_at.clone())?,
        canonical_url: announcement_url(
            &row.security_code,
            &row.organization_id,
            &row.announcement_id,
            &row.published_at,
        )?,
        pdf_url: row.adjunct_url.map(pdf_url).transpose()?,
        evidence,
    })
}

#[cfg(test)]
#[path = "../tests/unit/market_announcements_tests.rs"]
mod tests;
