use crate::{PbcError, PbcTableDescriptor, MAX_HTML_BYTES};
use encoding_rs::GB18030;
use magic_market_core::{
    DataBatch, EconomicObservation, EconomicObservationStatus, EconomicPeriod, EconomicRevision,
    EconomicSeriesRequest, FiniteNumber, NonEmptyText, Provenance, ProviderId, SourceEvidence,
};
use std::collections::{HashMap, HashSet};

const MAX_ROWS: usize = 100;
const MAX_COLUMNS: usize = 16;
const MAX_CELL_CHARS: usize = 512;

#[derive(Debug, Clone)]
struct Cell {
    text: String,
    rowspan: usize,
    colspan: usize,
}

#[derive(Debug, Clone)]
struct ExpandedCell {
    text: String,
    origin_row: usize,
    origin_column: usize,
    rowspan: usize,
    colspan: usize,
}

pub fn parse_money_supply_table(
    body: &[u8],
    descriptor: &PbcTableDescriptor,
    request: &EconomicSeriesRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<DataBatch<EconomicObservation>, PbcError> {
    parse_money_supply_response(body, None, descriptor, request, observed_at, batch_id)
}

pub(crate) fn parse_money_supply_response(
    body: &[u8],
    content_type: Option<&str>,
    descriptor: &PbcTableDescriptor,
    request: &EconomicSeriesRequest,
    observed_at: &str,
    batch_id: &str,
) -> Result<DataBatch<EconomicObservation>, PbcError> {
    crate::validate_request(request)?;
    let (request_start_year, _) = request
        .start()
        .as_month()
        .ok_or_else(|| PbcError::InvalidRequest("monthly range required".into()))?;
    let (request_end_year, _) = request
        .end()
        .as_month()
        .ok_or_else(|| PbcError::InvalidRequest("monthly range required".into()))?;
    if request_start_year != descriptor.year() as u32
        || request_end_year != descriptor.year() as u32
    {
        return Err(PbcError::InvalidRequest(
            "request month range must belong to the descriptor year".into(),
        ));
    }
    if body.len() > MAX_HTML_BYTES {
        return Err(PbcError::Protocol("HTML response exceeds 2 MiB".into()));
    }
    let html = decode_official_html(body, content_type)?;
    let tables = table_slices(&html)?;
    let mut primary_indexes = Vec::new();
    for (index, table) in tables.iter().enumerate() {
        if has_exact_title_pair(table, descriptor)? {
            primary_indexes.push(index);
        }
    }
    if primary_indexes.len() != 1 {
        return Err(PbcError::Protocol(
            "exactly one table with the audited bilingual title rows must be present".into(),
        ));
    }
    let primary_index = primary_indexes[0];
    let primary = tables[primary_index];
    validate_allowed_tags(primary)?;
    let grid = parse_grid(primary)?;
    let (months, rows) = parse_official_grid(&grid, descriptor)?;
    let revision = None;

    let requested: HashMap<&str, _> = request
        .series()
        .iter()
        .map(|series| (series.code(), series.clone()))
        .collect();
    let evidence = SourceEvidence::new(ProviderId::Pbc, observed_at, batch_id)?;
    let mut records = Vec::new();
    for code in ["M0", "M1", "M2"] {
        let Some(key) = requested.get(code) else {
            continue;
        };
        let (label, values) = rows
            .get(code)
            .ok_or_else(|| PbcError::Protocol("validated row disappeared".into()))?;
        for (period, value) in months.iter().zip(values) {
            if period < request.start() || period > request.end() {
                continue;
            }
            let status = if value.is_some() {
                EconomicObservationStatus::Present
            } else {
                EconomicObservationStatus::Missing
            };
            records.push(EconomicObservation::new(
                key.clone(),
                (*label).to_owned(),
                None,
                None,
                period.clone(),
                *value,
                descriptor.unit_zh(),
                Some(NonEmptyText::new("100 million yuan")?),
                None,
                status,
                None,
                applicable_revision(code, period, revision.as_ref()),
                evidence.clone(),
            )?);
        }
    }
    records.sort_by(|left, right| {
        left.series()
            .code()
            .cmp(right.series().code())
            .then_with(|| left.period().cmp(right.period()))
    });
    records.truncate(request.max_rows().get() as usize);
    if records.is_empty() {
        return Err(PbcError::Protocol(
            "money-supply request produced no rows after descriptor-bound filtering".into(),
        ));
    }
    let provenance =
        Provenance::new("People's Bank of China", observed_at)?.with_batch_id(batch_id)?;
    Ok(DataBatch::strict(records, provenance))
}

fn decode_official_html(body: &[u8], content_type: Option<&str>) -> Result<String, PbcError> {
    match declared_charset(content_type)? {
        Some(HtmlCharset::Utf8) => decode_utf8(body),
        Some(HtmlCharset::Gb18030) => decode_gb18030(body),
        None => decode_utf8(body).or_else(|_| decode_gb18030(body)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HtmlCharset {
    Utf8,
    Gb18030,
}

fn declared_charset(content_type: Option<&str>) -> Result<Option<HtmlCharset>, PbcError> {
    let Some(content_type) = content_type else {
        return Ok(None);
    };
    let mut declared = None;
    for parameter in content_type.split(';').skip(1) {
        let Some((name, value)) = parameter.split_once('=') else {
            continue;
        };
        if !name.trim().eq_ignore_ascii_case("charset") {
            continue;
        }
        if declared.is_some() {
            return Err(PbcError::Decode(
                "PBC response declares duplicate charset parameters".into(),
            ));
        }
        let value = value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_ascii_lowercase();
        declared = Some(match value.as_str() {
            "utf-8" | "utf8" => HtmlCharset::Utf8,
            "gbk" | "gb2312" | "gb18030" => HtmlCharset::Gb18030,
            _ => {
                return Err(PbcError::Decode(format!(
                    "PBC response declares unsupported charset {value:?}"
                )));
            }
        });
    }
    Ok(declared)
}

fn decode_utf8(body: &[u8]) -> Result<String, PbcError> {
    let decoded = std::str::from_utf8(body)
        .map_err(|_| PbcError::Decode("PBC HTML is not valid UTF-8".into()))?;
    Ok(decoded
        .strip_prefix('\u{feff}')
        .unwrap_or(decoded)
        .to_owned())
}

fn decode_gb18030(body: &[u8]) -> Result<String, PbcError> {
    let (decoded, _, had_errors) = GB18030.decode(body);
    if had_errors {
        return Err(PbcError::Decode(
            "PBC HTML contains invalid GBK/GB18030 byte sequences".into(),
        ));
    }
    if decoded.contains('\u{fffd}') {
        return Err(PbcError::Decode(
            "PBC HTML decoding produced replacement characters".into(),
        ));
    }
    Ok(decoded.into_owned())
}

fn table_slices(input: &str) -> Result<Vec<&str>, PbcError> {
    let lower = input.to_ascii_lowercase();
    let mut output = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = lower[cursor..].find("<table") {
        let start = cursor + relative;
        let open_end = find_tag_end(input, start)? + 1;
        let close = lower[open_end..]
            .find("</table>")
            .map(|offset| open_end + offset + "</table>".len())
            .ok_or_else(|| PbcError::Protocol("unterminated table".into()))?;
        output.push(&input[start..close]);
        cursor = close;
    }
    Ok(output)
}

fn has_exact_title_pair(table: &str, descriptor: &PbcTableDescriptor) -> Result<bool, PbcError> {
    let rows = tagged_slices(table, "tr")?;
    if rows.len() < 2 {
        return Ok(false);
    }
    Ok(
        is_physical_title_row(&parse_cells(rows[0])?, descriptor.title_zh())
            && is_physical_title_row(&parse_cells(rows[1])?, descriptor.title_en()),
    )
}

fn is_physical_title_row(cells: &[Cell], expected: &str) -> bool {
    cells.len() == 2
        && cells[0].text == expected
        && cells[0].rowspan == 1
        && cells[0].colspan == 15
        && cells[1].text.is_empty()
        && cells[1].rowspan == 1
        && cells[1].colspan == 1
}

type MoneySupplyRows = HashMap<&'static str, (&'static str, Vec<Option<FiniteNumber>>)>;

fn parse_official_grid(
    grid: &[Vec<ExpandedCell>],
    descriptor: &PbcTableDescriptor,
) -> Result<(Vec<EconomicPeriod>, MoneySupplyRows), PbcError> {
    if grid.len() != 19 || grid.iter().any(|row| row.len() != MAX_COLUMNS) {
        return Err(PbcError::Protocol(
            "official money-supply table must be exactly 19 rows by 16 columns".into(),
        ));
    }
    validate_merged_text_row(&grid[0], 0, descriptor.title_zh(), "Chinese title")?;
    validate_merged_text_row(&grid[1], 1, descriptor.title_en(), "English title")?;
    validate_unit_row(
        &grid[2],
        2,
        &format!("单位：{}", descriptor.unit_zh()),
        "Chinese",
    )?;
    validate_unit_row(
        &grid[3],
        3,
        &format!("Unit:{}", descriptor.unit_en()),
        "English",
    )?;
    validate_independent_blank_row(&grid[4], 4, "pre-header spacer")?;
    let months = validate_month_header(&grid[5], descriptor)?;
    validate_header_spacer(&grid[6], 6)?;

    let mut rows = HashMap::new();
    for layout in [
        SeriesLayout {
            code: "M2",
            zh_row: 7,
            en_row: 8,
            label_column: 0,
            label_span: 3,
            zh_label: "货币和准货币（M2）",
            en_label: "Money & Quasi-money",
        },
        SeriesLayout {
            code: "M1",
            zh_row: 9,
            en_row: 10,
            label_column: 1,
            label_span: 2,
            zh_label: "货币（M1）",
            en_label: "Money",
        },
        SeriesLayout {
            code: "M0",
            zh_row: 11,
            en_row: 12,
            label_column: 2,
            label_span: 1,
            zh_label: "流通中货币（M0）",
            en_label: "Currency in Circulation",
        },
    ] {
        let values = validate_series_pair(grid, layout)?;
        rows.insert(layout.code, (canonical_label(layout.code), values));
    }
    validate_note_and_history_suffix(grid)?;
    Ok((months, rows))
}

fn validate_merged_text_row(
    row: &[ExpandedCell],
    row_index: usize,
    expected: &str,
    label: &str,
) -> Result<(), PbcError> {
    if row[..15]
        .iter()
        .any(|cell| cell.text != expected || !canonical_origin(cell, row_index, 0, 1, 15))
        || !independent_blank(&row[15], row_index, 15)
    {
        return Err(PbcError::Protocol(format!(
            "{label} row differs from the audited merged-title shape"
        )));
    }
    Ok(())
}

fn validate_unit_row(
    row: &[ExpandedCell],
    row_index: usize,
    expected: &str,
    language: &str,
) -> Result<(), PbcError> {
    if row[..10]
        .iter()
        .enumerate()
        .any(|(column, cell)| !independent_blank(cell, row_index, column))
        || row[10..15]
            .iter()
            .any(|cell| cell.text != expected || !canonical_origin(cell, row_index, 10, 1, 5))
        || !independent_blank(&row[15], row_index, 15)
    {
        return Err(PbcError::Protocol(format!(
            "{language} unit row differs from the audited Excel layout"
        )));
    }
    Ok(())
}

fn validate_independent_blank_row(
    row: &[ExpandedCell],
    row_index: usize,
    label: &str,
) -> Result<(), PbcError> {
    if row
        .iter()
        .enumerate()
        .any(|(column, cell)| !independent_blank(cell, row_index, column))
    {
        return Err(PbcError::Protocol(format!(
            "{label} must be an independent all-blank 16-column row"
        )));
    }
    Ok(())
}

fn validate_month_header(
    row: &[ExpandedCell],
    descriptor: &PbcTableDescriptor,
) -> Result<Vec<EconomicPeriod>, PbcError> {
    if row[..3]
        .iter()
        .any(|cell| cell.text != "项目 Item" || !canonical_origin(cell, 5, 0, 1, 3))
        || !independent_blank(&row[15], 5, 15)
    {
        return Err(PbcError::Protocol(
            "month header does not expose the exact item hierarchy and empty tail".into(),
        ));
    }
    let months = row[3..15]
        .iter()
        .enumerate()
        .map(|(offset, cell)| {
            if !canonical_origin(cell, 5, 3 + offset, 1, 1) {
                return Err(PbcError::Protocol(
                    "month header cell has non-canonical span provenance".into(),
                ));
            }
            parse_header_month(&cell.text)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if months.len() != 12
        || months.iter().collect::<HashSet<_>>().len() != 12
        || months.iter().enumerate().any(|(index, month)| {
            month.as_month() != Some((descriptor.year() as u32, index as u32 + 1))
        })
    {
        return Err(PbcError::Protocol(
            "month header must be the descriptor year's exact January-through-December sequence"
                .into(),
        ));
    }
    Ok(months)
}

fn validate_header_spacer(row: &[ExpandedCell], row_index: usize) -> Result<(), PbcError> {
    if row[..3]
        .iter()
        .any(|cell| !cell.text.is_empty() || !canonical_origin(cell, row_index, 0, 1, 3))
        || row[3..]
            .iter()
            .enumerate()
            .any(|(offset, cell)| !independent_blank(cell, row_index, 3 + offset))
    {
        return Err(PbcError::Protocol(
            "post-header spacer differs from the audited Excel layout".into(),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct SeriesLayout {
    code: &'static str,
    zh_row: usize,
    en_row: usize,
    label_column: usize,
    label_span: usize,
    zh_label: &'static str,
    en_label: &'static str,
}

fn validate_series_pair(
    grid: &[Vec<ExpandedCell>],
    layout: SeriesLayout,
) -> Result<Vec<Option<FiniteNumber>>, PbcError> {
    let zh = &grid[layout.zh_row];
    let en = &grid[layout.en_row];
    for (row, row_index, expected) in [
        (zh, layout.zh_row, layout.zh_label),
        (en, layout.en_row, layout.en_label),
    ] {
        if row[..layout.label_column]
            .iter()
            .enumerate()
            .any(|(column, cell)| !independent_blank(cell, row_index, column))
            || row[layout.label_column..3].iter().any(|cell| {
                cell.text != expected
                    || !canonical_origin(cell, row_index, layout.label_column, 1, layout.label_span)
            })
            || !independent_blank(&row[15], row_index, 15)
        {
            return Err(PbcError::Protocol(format!(
                "{} {expected:?} bilingual hierarchy row differs from the audited layout",
                layout.code
            )));
        }
    }
    zh[3..15]
        .iter()
        .zip(&en[3..15])
        .enumerate()
        .map(|(offset, (zh_cell, en_cell))| {
            let column = 3 + offset;
            if !canonical_origin(zh_cell, layout.zh_row, column, 2, 1)
                || !same_origin(zh_cell, en_cell)
            {
                return Err(PbcError::Protocol(format!(
                    "{} value cell has non-canonical rowspan=2 provenance",
                    layout.code
                )));
            }
            parse_value(&zh_cell.text)
        })
        .collect()
}

fn validate_note_and_history_suffix(grid: &[Vec<ExpandedCell>]) -> Result<(), PbcError> {
    const ZH_NOTE: &str = "注：自2022年12月起，“流通中货币（M0）”含流通中数字人民币。12月末流通中数字人民币余额为136.1亿元。修订后，2022年各月末M1、M2增速无明显变化。修订后“流通中货币（M0）”增速如下：";
    const EN_NOTE: &str = "From December 2022, e-CNY in Circulation is covered in Currency in Circulation.The amount of e-CNY in Circulation at end December 2022 is RMB 13.61 billion. After applying the new method, M1 and M2 growth rates remain unchanged, M0 growth rates in 2022 are updated correspondingly:";

    validate_independent_blank_row(&grid[13], 13, "post-series spacer")?;
    validate_merged_text_row(&grid[14], 14, ZH_NOTE, "Chinese note")?;
    validate_merged_text_row(&grid[15], 15, EN_NOTE, "English note")?;

    for (column, cell) in grid[16][..3].iter().enumerate() {
        if !independent_blank(cell, 16, column) {
            return Err(PbcError::Protocol(
                "historical note header prefix must be blank".into(),
            ));
        }
    }
    for (offset, cell) in grid[16][3..15].iter().enumerate() {
        if !canonical_origin(cell, 16, 3 + offset, 1, 1)
            || parse_header_month(&cell.text)?.as_month() != Some((2022, offset as u32 + 1))
        {
            return Err(PbcError::Protocol(
                "historical note header must be exact 2022 January-through-December".into(),
            ));
        }
    }
    if !independent_blank(&grid[16][15], 16, 15)
        || !independent_blank(&grid[17][0], 17, 0)
        || !independent_blank(&grid[17][1], 17, 1)
        || grid[17][2].text != "流通中货币（ M0 ）"
        || !canonical_origin(&grid[17][2], 17, 2, 1, 1)
        || !independent_blank(&grid[17][15], 17, 15)
    {
        return Err(PbcError::Protocol(
            "historical M0 note row differs from the audited identity layout".into(),
        ));
    }
    for (offset, cell) in grid[17][3..15].iter().enumerate() {
        if !canonical_origin(cell, 17, 3 + offset, 1, 1) {
            return Err(PbcError::Protocol(
                "historical M0 percentage cell has non-canonical provenance".into(),
            ));
        }
        parse_percent(&cell.text)?;
    }
    validate_independent_blank_row(&grid[18], 18, "conditional width row")
}

fn parse_percent(input: &str) -> Result<(), PbcError> {
    let numeric = input
        .strip_suffix('%')
        .ok_or_else(|| PbcError::Protocol("growth footnote must use percent units".into()))?;
    let value = numeric
        .parse::<f64>()
        .map_err(|_| PbcError::Protocol("growth footnote value is not numeric".into()))?;
    FiniteNumber::new(value)?;
    Ok(())
}

fn parse_grid(table: &str) -> Result<Vec<Vec<ExpandedCell>>, PbcError> {
    let rows = tagged_slices(table, "tr")?;
    if rows.is_empty() || rows.len() > MAX_ROWS {
        return Err(PbcError::Protocol("table row ceiling violated".into()));
    }
    let mut grid: Vec<Vec<Option<ExpandedCell>>> = Vec::new();
    for (row_index, row_html) in rows.iter().enumerate() {
        if grid.len() <= row_index {
            grid.push(Vec::new());
        }
        let cells = parse_cells(row_html)?;
        let mut column = 0;
        for cell in cells {
            while grid[row_index].get(column).is_some_and(Option::is_some) {
                column += 1;
            }
            if column + cell.colspan > MAX_COLUMNS || row_index + cell.rowspan > MAX_ROWS {
                return Err(PbcError::Protocol("table span exceeds bounds".into()));
            }
            while grid.len() < row_index + cell.rowspan {
                grid.push(Vec::new());
            }
            let expanded = ExpandedCell {
                text: cell.text,
                origin_row: row_index,
                origin_column: column,
                rowspan: cell.rowspan,
                colspan: cell.colspan,
            };
            for target_row in grid.iter_mut().skip(row_index).take(cell.rowspan) {
                if target_row.len() < column + cell.colspan {
                    target_row.resize(column + cell.colspan, None);
                }
                for slot in target_row.iter_mut().skip(column).take(cell.colspan) {
                    if slot.is_some() {
                        return Err(PbcError::Protocol("overlapping table span".into()));
                    }
                    *slot = Some(expanded.clone());
                }
            }
            column += cell.colspan;
        }
    }
    let width = grid.iter().map(Vec::len).max().unwrap_or(0);
    if width == 0 || width > MAX_COLUMNS {
        return Err(PbcError::Protocol("invalid table width".into()));
    }
    grid.into_iter()
        .map(|mut row| {
            row.resize(width, None);
            row.into_iter()
                .map(|cell| {
                    cell.ok_or_else(|| PbcError::Protocol("non-rectangular table layout".into()))
                })
                .collect()
        })
        .collect()
}

fn parse_cells(row: &str) -> Result<Vec<Cell>, PbcError> {
    let lower = row.to_ascii_lowercase();
    let mut cells = Vec::new();
    let mut cursor = 0;
    while cursor < row.len() {
        let td = lower[cursor..].find("<td").map(|offset| (offset, "td"));
        let th = lower[cursor..].find("<th").map(|offset| (offset, "th"));
        let Some((relative, tag)) = [td, th].into_iter().flatten().min_by_key(|item| item.0) else {
            break;
        };
        let start = cursor + relative;
        let open_end = find_tag_end(row, start)?;
        let attrs = &row[start + tag.len() + 1..open_end];
        let closing = format!("</{tag}>");
        let close_start = lower[open_end + 1..]
            .find(&closing)
            .map(|offset| open_end + 1 + offset)
            .ok_or_else(|| PbcError::Protocol("unterminated table cell content".into()))?;
        let text = normalize(&strip_tags(&row[open_end + 1..close_start])?);
        if text.chars().count() > MAX_CELL_CHARS {
            return Err(PbcError::Protocol(
                "table cell text ceiling exceeded".into(),
            ));
        }
        let (rowspan, colspan) = parse_cell_spans(attrs)?;
        cells.push(Cell {
            text,
            rowspan,
            colspan,
        });
        cursor = close_start + closing.len();
    }
    Ok(cells)
}

fn parse_cell_spans(attrs: &str) -> Result<(usize, usize), PbcError> {
    let mut cursor = 0;
    let mut rowspan = None;
    let mut colspan = None;
    while cursor < attrs.len() {
        let separator = attrs[cursor..]
            .find(|character: char| !character.is_whitespace())
            .unwrap_or(attrs.len() - cursor);
        if separator == 0 {
            return Err(PbcError::Protocol(
                "cell attributes must be separated by whitespace".into(),
            ));
        }
        cursor += separator;
        if cursor == attrs.len() {
            break;
        }
        let name_start = cursor;
        while cursor < attrs.len()
            && attrs.as_bytes()[cursor].is_ascii()
            && is_attribute_name_byte(attrs.as_bytes()[cursor])
        {
            cursor += 1;
        }
        if cursor == name_start {
            return Err(PbcError::Protocol(
                "cell attribute name contains unsupported syntax".into(),
            ));
        }
        let name = attrs[name_start..cursor].to_ascii_lowercase();
        while cursor < attrs.len() && attrs.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let value = if cursor < attrs.len() && attrs.as_bytes()[cursor] == b'=' {
            cursor += 1;
            while cursor < attrs.len() && attrs.as_bytes()[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            Some(parse_attribute_value(attrs, &mut cursor)?)
        } else {
            None
        };
        match name.as_str() {
            "rowspan" => {
                if rowspan.is_some() {
                    return Err(PbcError::Protocol("duplicate rowspan attribute".into()));
                }
                rowspan = Some(parse_span_token("rowspan", value)?);
            }
            "colspan" => {
                if colspan.is_some() {
                    return Err(PbcError::Protocol("duplicate colspan attribute".into()));
                }
                colspan = Some(parse_span_token("colspan", value)?);
            }
            _ => {}
        }
    }
    Ok((rowspan.unwrap_or(1), colspan.unwrap_or(1)))
}

fn is_attribute_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.')
}

fn parse_attribute_value<'a>(attrs: &'a str, cursor: &mut usize) -> Result<&'a str, PbcError> {
    if *cursor >= attrs.len() {
        return Err(PbcError::Protocol(
            "cell attribute is missing its value".into(),
        ));
    }
    let quote = attrs.as_bytes()[*cursor];
    if matches!(quote, b'"' | b'\'') {
        *cursor += 1;
        let start = *cursor;
        while *cursor < attrs.len() && attrs.as_bytes()[*cursor] != quote {
            *cursor += 1;
        }
        if *cursor == attrs.len() {
            return Err(PbcError::Protocol(
                "cell attribute has unterminated quotes".into(),
            ));
        }
        let value = &attrs[start..*cursor];
        *cursor += 1;
        Ok(value)
    } else {
        let start = *cursor;
        while *cursor < attrs.len() && !attrs.as_bytes()[*cursor].is_ascii_whitespace() {
            *cursor += 1;
        }
        if start == *cursor {
            return Err(PbcError::Protocol(
                "cell attribute is missing its value".into(),
            ));
        }
        Ok(&attrs[start..*cursor])
    }
}

fn parse_span_token(name: &str, value: Option<&str>) -> Result<usize, PbcError> {
    let value = value.ok_or_else(|| PbcError::Protocol(format!("{name} requires a value")))?;
    let value = value
        .parse::<usize>()
        .map_err(|_| PbcError::Protocol(format!("invalid {name} value")))?;
    if !(1..=MAX_COLUMNS).contains(&value) {
        return Err(PbcError::Protocol(format!("{name} is outside bounds")));
    }
    Ok(value)
}

fn tagged_slices<'a>(input: &'a str, tag: &str) -> Result<Vec<&'a str>, PbcError> {
    let lower = input.to_ascii_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut slices = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = lower[cursor..].find(&open) {
        let start = cursor + relative;
        let content_start = find_tag_end(input, start)? + 1;
        let content_end = lower[content_start..]
            .find(&close)
            .map(|offset| content_start + offset)
            .ok_or_else(|| PbcError::Protocol(format!("unterminated {tag} element")))?;
        slices.push(&input[content_start..content_end]);
        cursor = content_end + close.len();
    }
    Ok(slices)
}

fn strip_tags(input: &str) -> Result<String, PbcError> {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;
    for character in input.chars() {
        match character {
            '<' if !in_tag => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                output.push(' ');
            }
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }
    if in_tag {
        return Err(PbcError::Protocol("malformed HTML tag".into()));
    }
    decode_entities(&output)
}

fn validate_allowed_tags(input: &str) -> Result<(), PbcError> {
    let mut stack: Vec<String> = Vec::new();
    let mut excel_condition_open = false;
    let mut excel_condition_seen = false;
    let mut conditional_row = None;
    let mut row_count = 0;
    let mut cursor = 0;
    while let Some(relative) = input[cursor..].find('<') {
        let start = cursor + relative;
        validate_direct_text(&input[cursor..start], stack.last().map(String::as_str))?;
        let end = find_tag_end(input, start)?;
        let raw = input[start + 1..end].trim();
        if raw == "![if supportMisalignedColumns]" {
            if excel_condition_open
                || excel_condition_seen
                || stack.last().map(String::as_str) != Some("table")
            {
                return Err(PbcError::Protocol(
                    "Excel conditional-width directive is misplaced or duplicated".into(),
                ));
            }
            excel_condition_open = true;
            excel_condition_seen = true;
            cursor = end + 1;
            continue;
        }
        if raw == "![endif]" {
            if !excel_condition_open || stack.last().map(String::as_str) != Some("table") {
                return Err(PbcError::Protocol(
                    "Excel conditional-width terminator is misplaced".into(),
                ));
            }
            excel_condition_open = false;
            cursor = end + 1;
            continue;
        }
        if raw.is_empty() || raw.starts_with('!') || raw.starts_with('?') {
            return Err(PbcError::Protocol(
                "unsupported or empty table markup".into(),
            ));
        }
        let closing = raw.starts_with('/');
        let self_closing = raw.ends_with('/');
        if self_closing {
            return Err(PbcError::Protocol(
                "self-closing table elements are forbidden".into(),
            ));
        }
        let raw_name = raw.trim_start_matches('/').trim_start();
        let name_end = raw
            .trim_start_matches('/')
            .trim_start()
            .find(char::is_whitespace)
            .unwrap_or(raw_name.len());
        let name = raw_name[..name_end].to_ascii_lowercase();
        if !matches!(
            name.as_str(),
            "table" | "caption" | "col" | "tr" | "th" | "td" | "sup" | "p" | "span" | "font" | "br"
        ) {
            return Err(PbcError::Protocol(format!(
                "unsupported table element {name:?}"
            )));
        }
        let void = matches!(name.as_str(), "col" | "br");
        if closing {
            if void {
                return Err(PbcError::Protocol(format!(
                    "void table element {name:?} must not have a closing tag"
                )));
            }
            if !raw_name[name_end..].trim().is_empty() {
                return Err(PbcError::Protocol(
                    "closing table tag contains unexpected attributes".into(),
                ));
            }
            let open = stack.pop().ok_or_else(|| {
                PbcError::Protocol(format!("stray closing table element {name:?}"))
            })?;
            if open != name {
                return Err(PbcError::Protocol(format!(
                    "misnested table markup: closed {name:?} while {open:?} was open"
                )));
            }
        } else {
            validate_parent(&name, stack.last().map(String::as_str))?;
            if name == "tr" {
                if excel_condition_open && conditional_row.replace(row_count).is_some() {
                    return Err(PbcError::Protocol(
                        "Excel conditional-width block contains multiple rows".into(),
                    ));
                }
                row_count += 1;
            }
            if !void {
                stack.push(name);
            }
        }
        cursor = end + 1;
    }
    validate_direct_text(&input[cursor..], stack.last().map(String::as_str))?;
    if !stack.is_empty()
        || excel_condition_open
        || !excel_condition_seen
        || row_count != 19
        || conditional_row != Some(18)
    {
        return Err(PbcError::Protocol(
            "table markup or final Excel conditional-width row differs from the audited shape"
                .into(),
        ));
    }
    Ok(())
}

fn find_tag_end(input: &str, start: usize) -> Result<usize, PbcError> {
    let mut quote = None;
    for (offset, character) in input[start + 1..].char_indices() {
        match (quote, character) {
            (Some(active), current) if active == current => quote = None,
            (None, '"' | '\'') => quote = Some(character),
            (None, '>') => return Ok(start + 1 + offset),
            _ => {}
        }
    }
    Err(PbcError::Protocol("unterminated HTML element".into()))
}

fn validate_parent(name: &str, parent: Option<&str>) -> Result<(), PbcError> {
    let valid = match name {
        "table" => parent.is_none(),
        "caption" | "col" | "tr" => parent == Some("table"),
        "th" | "td" => parent == Some("tr"),
        "p" => matches!(parent, Some("caption" | "th" | "td")),
        "sup" | "span" | "font" => matches!(
            parent,
            Some("caption" | "th" | "td" | "p" | "sup" | "span" | "font")
        ),
        "br" => matches!(
            parent,
            Some("caption" | "th" | "td" | "p" | "sup" | "span" | "font")
        ),
        _ => false,
    };
    if !valid {
        return Err(PbcError::Protocol(format!(
            "stray or misnested table element {name:?} under {parent:?}"
        )));
    }
    Ok(())
}

fn validate_direct_text(text: &str, parent: Option<&str>) -> Result<(), PbcError> {
    if text.trim().is_empty()
        || matches!(
            parent,
            Some("caption" | "th" | "td" | "p" | "sup" | "span" | "font")
        )
    {
        return Ok(());
    }
    Err(PbcError::Protocol(format!(
        "stray text appears directly under table context {parent:?}"
    )))
}

fn decode_entities(input: &str) -> Result<String, PbcError> {
    let mut output = input.to_owned();
    for (encoded, decoded) in [
        ("&nbsp;", " "),
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
    ] {
        output = output.replace(encoded, decoded);
    }
    if output.contains('&') && output.split('&').skip(1).any(|part| part.contains(';')) {
        return Err(PbcError::Protocol("unsupported HTML entity".into()));
    }
    Ok(output)
}

fn normalize(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_header_month(value: &str) -> Result<EconomicPeriod, PbcError> {
    let value = normalize(value);
    if value.len() != 7
        || value.as_bytes().get(4) != Some(&b'.')
        || !value[..4].bytes().all(|byte| byte.is_ascii_digit())
        || !value[5..].bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(PbcError::Protocol(format!(
            "invalid month header {value:?}"
        )));
    }
    EconomicPeriod::month(
        value[..4]
            .parse()
            .map_err(|_| PbcError::Protocol("invalid header year".into()))?,
        value[5..]
            .parse()
            .map_err(|_| PbcError::Protocol("invalid header month".into()))?,
    )
    .map_err(PbcError::from)
}

fn canonical_origin(
    cell: &ExpandedCell,
    origin_row: usize,
    origin_column: usize,
    rowspan: usize,
    colspan: usize,
) -> bool {
    cell.origin_row == origin_row
        && cell.origin_column == origin_column
        && cell.rowspan == rowspan
        && cell.colspan == colspan
}

fn same_origin(left: &ExpandedCell, right: &ExpandedCell) -> bool {
    left.origin_row == right.origin_row
        && left.origin_column == right.origin_column
        && left.rowspan == right.rowspan
        && left.colspan == right.colspan
}

fn independent_blank(cell: &ExpandedCell, row: usize, column: usize) -> bool {
    cell.text.is_empty() && canonical_origin(cell, row, column, 1, 1)
}

fn canonical_label(code: &str) -> &'static str {
    match code {
        "M2" => "货币和准货币（M2） / Money & Quasi-money",
        "M1" => "货币（M1） / Money",
        "M0" => "流通中货币（M0） / Currency in Circulation",
        _ => unreachable!("validated hierarchy code"),
    }
}

fn parse_value(input: &str) -> Result<Option<FiniteNumber>, PbcError> {
    let value = normalize(input).replace(',', "");
    if value.is_empty() || value == "—" {
        return Ok(None);
    }
    let parsed = value
        .parse::<f64>()
        .map_err(|_| PbcError::Protocol(format!("invalid money-supply value {value:?}")))?;
    if parsed.is_sign_negative() {
        return Err(PbcError::Protocol(
            "money-supply balance must be non-negative".into(),
        ));
    }
    Ok(Some(FiniteNumber::new(parsed)?))
}

fn applicable_revision(
    code: &str,
    period: &EconomicPeriod,
    revision: Option<&EconomicRevision>,
) -> Option<EconomicRevision> {
    if code == "M1"
        && period
            .as_month()
            .is_some_and(|(year, month)| (year, month) >= (2025, 1))
    {
        return revision.cloned();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn methodology_revision_is_scoped_to_m1_from_2025() {
        let revision = EconomicRevision {
            kind: magic_market_core::EconomicRevisionKind::SourceDefined(
                NonEmptyText::new("source-defined").unwrap(),
            ),
            label: None,
        };
        assert!(applicable_revision(
            "M1",
            &EconomicPeriod::month(2025, 1).unwrap(),
            Some(&revision)
        )
        .is_some());
        assert!(applicable_revision(
            "M0",
            &EconomicPeriod::month(2025, 1).unwrap(),
            Some(&revision)
        )
        .is_none());
        assert!(applicable_revision(
            "M1",
            &EconomicPeriod::month(2024, 12).unwrap(),
            Some(&revision)
        )
        .is_none());
    }
}
