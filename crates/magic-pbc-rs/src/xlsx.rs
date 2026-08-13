use crate::{validate_regional_request, PbcError, MAX_REGIONAL_XLSX_BYTES};
use calamine::{Data, Reader, SheetType, SheetVisible, Xlsx};
use magic_market_core::{
    DataBatch, EconomicObservation, EconomicObservationStatus, EconomicPeriod, EconomicRevision,
    EconomicRevisionKind, EconomicSeriesRequest, FiniteNumber, NonEmptyText, Provenance,
    ProviderId, SourceEvidence,
};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;

const MIME: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
const SHEET: &str = "2025年一季度地区社会融资规模增量";
const TEMPLATE_SHEET: &str = "地区公布表式";
const TITLE_ZH: &str = "2025年一季度地区社会融资规模增量统计表";
const TITLE_EN: &str =
    "Aggregate Financing to the Real Economy（flow） by Province(The First Quarter of 2025)";
const UNIT_ZH: &str = "单位：亿元人民币";
const UNIT_EN: &str = "Unit: 100 Million Yuan";
const PRELIMINARY_NOTE_ZH: &str = "注2:表中数据为初步统计数。";
const PRELIMINARY_NOTE_EN: &str = "Note2: The statistics is provisional.";
const MAX_ARCHIVE_ENTRIES: usize = 64;
const MAX_ARCHIVE_ENTRY_BYTES: usize = 2 * 1024 * 1024;
const MAX_ARCHIVE_TOTAL_BYTES: usize = 4 * 1024 * 1024;

pub const REGIONAL_SOCIAL_FINANCING_CODES: [&str; 9] = [
    "AFRE_FLOW",
    "RMB_LOANS",
    "FOREIGN_CURRENCY_LOANS_RMB",
    "ENTRUSTED_LOANS",
    "TRUST_LOANS",
    "UNDISCOUNTED_BANKERS_ACCEPTANCES",
    "CORPORATE_BONDS",
    "GOVERNMENT_BONDS",
    "DOMESTIC_EQUITY_FINANCING",
];

const COLUMNS: [(&str, &str, &str); 9] = [
    (
        "AFRE_FLOW",
        "地区社会融资规模增量",
        "Aggregate financing to the real economy(flow) by province",
    ),
    ("RMB_LOANS", "人民币贷款", "RMB  loans"),
    (
        "FOREIGN_CURRENCY_LOANS_RMB",
        "外币贷款（折合人民币）",
        "Foreign currency-denominated loans (RMB equivalent)",
    ),
    ("ENTRUSTED_LOANS", "委托贷款", "Entrusted loans"),
    ("TRUST_LOANS", "信托贷款", "Trust loans"),
    (
        "UNDISCOUNTED_BANKERS_ACCEPTANCES",
        "未贴现银行承兑汇票",
        "Undiscounted bankers'  acceptances",
    ),
    (
        "CORPORATE_BONDS",
        "企业债券",
        "Net financing of corporate bonds",
    ),
    ("GOVERNMENT_BONDS", "政府债券", "Government bonds"),
    (
        "DOMESTIC_EQUITY_FINANCING",
        "非金融企业境内股票融资",
        "Equity financing on the domestic stock market by non-financial enterprises",
    ),
];

const REGIONS: [&str; 31] = [
    "北京 Beijing",
    "天津 Tianjin",
    "河北 Hebei",
    "山西 Shanxi",
    "内蒙古 Inner Mongolia",
    "辽宁 Liaoning",
    "吉林 Jilin",
    "黑龙江 Heilongjiang",
    "上海 Shanghai",
    "江苏 Jiangsu",
    "浙江 Zhejiang",
    "安徽 Anhui",
    "福建 Fujian",
    "江西 Jiangxi",
    "山东 Shandong",
    "河南 Henan",
    "湖北 Hubei",
    "湖南 Hunan",
    "广东 Guangdong",
    "广西 Guangxi",
    "海南 Hainan",
    "重庆 Chongqing",
    "四川 Sichuan",
    "贵州 Guizhou",
    "云南 Yunnan",
    "西藏 Tibet",
    "陕西 Shanxi",
    "甘肃 Gansu",
    "青海 Qinghai",
    "宁夏 Ningxia",
    "新疆 Xinjiang",
];

pub fn parse_regional_social_financing_workbook(
    body: &[u8],
    request: &EconomicSeriesRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<DataBatch<EconomicObservation>, PbcError> {
    parse_regional_social_financing_response(body, None, request, observed_at, batch_id)
}

pub(crate) fn parse_regional_social_financing_response(
    body: &[u8],
    content_type: Option<&str>,
    request: &EconomicSeriesRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<DataBatch<EconomicObservation>, PbcError> {
    validate_regional_request(request)?;
    if body.len() > MAX_REGIONAL_XLSX_BYTES {
        return Err(PbcError::Protocol("XLSX response exceeds 256 KiB".into()));
    }
    if let Some(value) = content_type {
        let media = value.split(';').next().unwrap_or("").trim();
        if !media.eq_ignore_ascii_case(MIME) {
            return Err(PbcError::Decode(format!(
                "PBC regional response declares unsupported media type {media:?}"
            )));
        }
    }
    validate_zip_bounds(body)?;
    let mut workbook: Xlsx<_> = Xlsx::new(Cursor::new(body))
        .map_err(|error| PbcError::Decode(format!("invalid XLSX workbook: {error}")))?;
    let sheet_metadata = workbook.sheets_metadata();
    if sheet_metadata.len() != 2
        || sheet_metadata[0].name != SHEET
        || sheet_metadata[0].typ != SheetType::WorkSheet
        || sheet_metadata[0].visible != SheetVisible::Visible
        || sheet_metadata[1].name != TEMPLATE_SHEET
        || sheet_metadata[1].typ != SheetType::WorkSheet
        || sheet_metadata[1].visible != SheetVisible::Hidden
    {
        return Err(PbcError::Protocol(
            "regional workbook sheet catalog or visibility differs from the audited two-sheet contract"
                .into(),
        ));
    }
    let range = workbook
        .worksheet_range(SHEET)
        .map_err(|error| PbcError::Decode(format!("regional sheet cannot be read: {error}")))?;
    require_text(&range, 1, 0, TITLE_ZH, true)?;
    require_text(&range, 2, 0, TITLE_EN, false)?;
    require_text(&range, 3, 0, UNIT_ZH, false)?;
    require_text(&range, 4, 0, UNIT_EN, false)?;
    require_text(&range, 42, 0, PRELIMINARY_NOTE_ZH, false)?;
    require_text(&range, 43, 0, PRELIMINARY_NOTE_EN, false)?;

    for (offset, (_, header_zh, header_en)) in COLUMNS.iter().enumerate() {
        let column = offset + 1;
        let header_row = if offset == 0 { 5 } else { 6 };
        require_text(&range, header_row, column, header_zh, true)?;
        let english_row = if offset == 0 { 6 } else { 7 };
        require_text(&range, english_row, column, header_en, false)?;
    }

    let requested: HashMap<&str, _> = request
        .series()
        .iter()
        .map(|series| (series.code(), series.clone()))
        .collect();
    let evidence = SourceEvidence::new(ProviderId::Pbc, observed_at, batch_id)?;
    let period = EconomicPeriod::quarter(2025, 1)?;
    let revision = EconomicRevision {
        kind: EconomicRevisionKind::Preliminary,
        label: Some(NonEmptyText::new(PRELIMINARY_NOTE_EN)?),
    };
    let scale = NonEmptyText::new("100 million yuan")?;
    let mut records = Vec::with_capacity(requested.len() * REGIONS.len());
    for (region_offset, expected_region) in REGIONS.iter().enumerate() {
        let row = 9 + region_offset;
        require_text(&range, row, 0, expected_region, false)?;
        for (column_offset, (code, name, _)) in COLUMNS.iter().enumerate() {
            let Some(series) = requested.get(code) else {
                continue;
            };
            let value = numeric_cell(&range, row, column_offset + 1)?;
            records.push(EconomicObservation::new(
                series.clone(),
                *name,
                None,
                Some(NonEmptyText::new(*expected_region)?),
                period.clone(),
                Some(value),
                "亿元人民币",
                Some(scale.clone()),
                None,
                EconomicObservationStatus::Present,
                None,
                Some(revision.clone()),
                evidence.clone(),
            )?);
        }
    }
    if records.len() != requested.len() * REGIONS.len() {
        return Err(PbcError::Protocol(
            "regional workbook did not yield complete requested-region coverage".into(),
        ));
    }
    records.sort_by(|left, right| {
        left.series()
            .code()
            .cmp(right.series().code())
            .then_with(|| left.region_name().cmp(&right.region_name()))
    });
    let provenance =
        Provenance::new("People's Bank of China", observed_at)?.with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn validate_zip_bounds(body: &[u8]) -> Result<(), PbcError> {
    const EOCD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
    const CENTRAL_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];
    const MIN_EOCD_BYTES: usize = 22;
    const MAX_COMMENT_BYTES: usize = u16::MAX as usize;
    if body.len() < MIN_EOCD_BYTES {
        return Err(PbcError::Decode("XLSX ZIP container is truncated".into()));
    }
    let search_start = body
        .len()
        .saturating_sub(MIN_EOCD_BYTES + MAX_COMMENT_BYTES);
    let eocd = (search_start..=body.len() - MIN_EOCD_BYTES)
        .rev()
        .find(|offset| body[*offset..].starts_with(&EOCD_SIGNATURE))
        .ok_or_else(|| PbcError::Decode("XLSX ZIP end record is missing".into()))?;
    let disk = read_u16(body, eocd + 4)?;
    let central_disk = read_u16(body, eocd + 6)?;
    let disk_entries = read_u16(body, eocd + 8)? as usize;
    let entries = read_u16(body, eocd + 10)? as usize;
    let central_bytes = read_u32(body, eocd + 12)? as usize;
    let central_offset = read_u32(body, eocd + 16)? as usize;
    let comment_bytes = read_u16(body, eocd + 20)? as usize;
    if eocd + MIN_EOCD_BYTES + comment_bytes != body.len()
        || disk != 0
        || central_disk != 0
        || disk_entries != entries
        || entries == 0
        || entries > MAX_ARCHIVE_ENTRIES
        || entries == u16::MAX as usize
        || central_bytes == u32::MAX as usize
        || central_offset == u32::MAX as usize
        || central_offset.checked_add(central_bytes) != Some(eocd)
    {
        return Err(PbcError::Protocol(
            "XLSX ZIP directory violates the bounded single-disk contract".into(),
        ));
    }

    let mut cursor = central_offset;
    let mut total_uncompressed = 0_usize;
    let mut names = HashSet::with_capacity(entries);
    for _ in 0..entries {
        if body.get(cursor..cursor + 4) != Some(CENTRAL_SIGNATURE.as_slice()) {
            return Err(PbcError::Decode(
                "XLSX ZIP central directory is malformed".into(),
            ));
        }
        let flags = read_u16(body, cursor + 8)?;
        let compression = read_u16(body, cursor + 10)?;
        let compressed = read_u32(body, cursor + 20)? as usize;
        let uncompressed = read_u32(body, cursor + 24)? as usize;
        let name_bytes = read_u16(body, cursor + 28)? as usize;
        let extra_bytes = read_u16(body, cursor + 30)? as usize;
        let comment_bytes = read_u16(body, cursor + 32)? as usize;
        let start_disk = read_u16(body, cursor + 34)?;
        let local_offset = read_u32(body, cursor + 42)? as usize;
        let header_bytes = 46_usize
            .checked_add(name_bytes)
            .and_then(|value| value.checked_add(extra_bytes))
            .and_then(|value| value.checked_add(comment_bytes))
            .ok_or_else(|| PbcError::Protocol("XLSX ZIP header length overflow".into()))?;
        let next = cursor
            .checked_add(header_bytes)
            .filter(|next| *next <= eocd)
            .ok_or_else(|| PbcError::Decode("XLSX ZIP header is truncated".into()))?;
        let name_start = cursor + 46;
        let name_end = name_start + name_bytes;
        let name = std::str::from_utf8(&body[name_start..name_end])
            .map_err(|_| PbcError::Decode("XLSX ZIP entry name is not UTF-8".into()))?;
        if flags & 1 != 0
            || !matches!(compression, 0 | 8)
            || start_disk != 0
            || compressed == u32::MAX as usize
            || uncompressed == u32::MAX as usize
            || uncompressed > MAX_ARCHIVE_ENTRY_BYTES
            || local_offset >= central_offset
            || name.is_empty()
            || name.len() > 256
            || name.starts_with('/')
            || name.contains('\\')
            || name.split('/').any(|part| part == "..")
            || !names.insert(name.to_owned())
        {
            return Err(PbcError::Protocol(
                "XLSX ZIP entry violates the bounded archive contract".into(),
            ));
        }
        total_uncompressed = total_uncompressed
            .checked_add(uncompressed)
            .filter(|total| *total <= MAX_ARCHIVE_TOTAL_BYTES)
            .ok_or_else(|| PbcError::Protocol("XLSX expanded content exceeds 4 MiB".into()))?;
        cursor = next;
    }
    if cursor != eocd
        || !names.contains("[Content_Types].xml")
        || !names.contains("xl/workbook.xml")
        || !names.contains("xl/worksheets/sheet1.xml")
    {
        return Err(PbcError::Protocol(
            "XLSX ZIP directory lacks the exact required workbook parts".into(),
        ));
    }
    Ok(())
}

fn read_u16(body: &[u8], offset: usize) -> Result<u16, PbcError> {
    let bytes: [u8; 2] = body
        .get(offset..offset + 2)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| PbcError::Decode("XLSX ZIP integer is truncated".into()))?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32(body: &[u8], offset: usize) -> Result<u32, PbcError> {
    let bytes: [u8; 4] = body
        .get(offset..offset + 4)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| PbcError::Decode("XLSX ZIP integer is truncated".into()))?;
    Ok(u32::from_le_bytes(bytes))
}

fn require_text(
    range: &calamine::Range<Data>,
    row: usize,
    column: usize,
    expected: &str,
    trim: bool,
) -> Result<(), PbcError> {
    let actual = match range.get_value((row as u32, column as u32)) {
        Some(Data::String(value)) => value.as_str(),
        _ => {
            return Err(PbcError::Protocol(format!(
                "regional workbook cell ({row},{column}) is not source text"
            )))
        }
    };
    let actual = if trim { actual.trim() } else { actual };
    if actual != expected {
        return Err(PbcError::Protocol(format!(
            "regional workbook cell ({row},{column}) differs from the audited contract"
        )));
    }
    Ok(())
}

fn numeric_cell(
    range: &calamine::Range<Data>,
    row: usize,
    column: usize,
) -> Result<FiniteNumber, PbcError> {
    let number = match range.get_value((row as u32, column as u32)) {
        Some(Data::Int(value)) => *value as f64,
        Some(Data::Float(value)) if value.fract() == 0.0 => *value,
        _ => {
            return Err(PbcError::Protocol(format!(
                "regional workbook cell ({row},{column}) is not a published integral value"
            )))
        }
    };
    FiniteNumber::new(number).map_err(PbcError::from)
}
