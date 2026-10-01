use crate::dom::Document;
use crate::{
    profiles, protocol, OfficialNewsError, OfficialPublication, OfficialSource,
    PublicationLabelOrigin, PublicationListing, PublicationPrecision, PublicationReference,
};
use magic_market_core::{HttpsUrl, IsoDate};
use std::collections::{HashMap, HashSet};
use url::Url;

pub(crate) fn listing(
    source: OfficialSource,
    html: &str,
    limit: u32,
    observed_at: String,
    hash: String,
) -> Result<PublicationListing, OfficialNewsError> {
    let profile = profiles::profile(source);
    let document = Document::parse(html)?;
    if source == OfficialSource::Nea {
        let groups = document.select(".xwzx-page01 .xwzx-yw-right .xwzx-yw-box > ul")?;
        let expected = [
            "datasource:91fb3999d1964141b668e4a4cef4ed98",
            "datasource:64763711c745408cb6f3bc0895f37649",
        ];
        if groups.len() != expected.len() {
            return Err(protocol(
                "NEA work-update source groups are missing or ambiguous",
            ));
        }
        for (group, id) in groups.iter().zip(expected) {
            if group.attr("data").as_deref() != Some(id) || group.select("li")?.len() != 5 {
                return Err(protocol(
                    "NEA work-update group identity or row count changed",
                ));
            }
        }
        let more = document.select(".xwzx-page01 .xwzx-yw-right > div.more_dw > a")?;
        if more.len() != 1 || more[0].attr("href").as_deref() != Some("/news/jwzdt.htm") {
            return Err(protocol("NEA work-update channel identity changed"));
        }
    }
    if source == OfficialSource::Miit {
        let containers = document.select("div.page-content")?;
        if containers.len() != 1
            || containers[0].parent()?.attr("id").as_deref() != Some("右侧内容")
            || containers[0].select("ul")?.len() != 1
        {
            return Err(protocol(
                "MIIT unit content identity is missing or ambiguous",
            ));
        }
    }
    let mut rows = Vec::new();
    for row in document.select(profile.rows)? {
        if source == OfficialSource::Ndrc
            && row
                .attr("class")
                .is_some_and(|value| value.split_whitespace().any(|part| part == "empty"))
        {
            if !row.select("a")?.is_empty() || !row.text()?.is_empty() {
                return Err(protocol(
                    "NDRC separator contains unexpected publication content",
                ));
            }
        } else {
            rows.push(row);
        }
    }
    let maximum_rows = if source == OfficialSource::Miit {
        24
    } else {
        100
    };
    if rows.is_empty() || rows.len() > maximum_rows {
        return Err(protocol("listing must contain 1 through 100 source rows"));
    }
    let base = Url::parse(profile.listing).map_err(|_| protocol("listing URL is invalid"))?;
    let mut urls = HashSet::new();
    let mut items = Vec::with_capacity(rows.len());
    for row in &rows {
        let anchors = row.select(profile.anchor)?;
        if anchors.len() != 1 {
            return Err(protocol(
                "listing row must expose exactly one primary article anchor",
            ));
        }
        let anchor = anchors[0];
        let title = match anchor.attr("title") {
            Some(title) => title,
            None => anchor.text()?,
        };
        let title = checked_text(&title, 512)?;
        let url = base
            .join(
                &anchor
                    .attr("href")
                    .ok_or_else(|| protocol("article href is missing"))?,
            )
            .map_err(|_| protocol("article href is invalid"))?;
        profiles::validate_article(source, url.as_str())?;
        if !urls.insert(url.to_string()) {
            return Err(protocol("duplicate source article URL"));
        }
        let date_row = if profile.date_on_parent {
            row.parent()?
        } else {
            *row
        };
        let dates = date_row.select("span")?;
        if dates.len() != 1 {
            return Err(protocol("listing row must expose exactly one date label"));
        }
        let raw_date = dates[0].text()?;
        let date = IsoDate::new(
            raw_date
                .trim_matches(['[', ']', '(', ')'])
                .replace('/', "-"),
        )?;
        validate_observed_date(&date, &observed_at)?;
        items.push(PublicationReference {
            source,
            title,
            canonical_url: HttpsUrl::new(url.to_string())?,
            published_date: date,
            publication_label: raw_date,
            precision: PublicationPrecision::Date,
            publication_label_origin: PublicationLabelOrigin::ListingHtml,
        });
    }
    let source_rows = items.len();
    if matches!(source, OfficialSource::Mofcom | OfficialSource::Miit) {
        let pagination = document.select("div.pagination")?;
        if pagination.len() != 1 {
            return Err(protocol("unit pagination metadata is missing or ambiguous"));
        }
        let number = |name: &str| -> Result<usize, OfficialNewsError> {
            pagination[0]
                .attr(name)
                .ok_or_else(|| protocol("unit pagination attribute is missing"))?
                .parse::<usize>()
                .map_err(|_| protocol("unit pagination number is invalid"))
        };
        if number("rows")? != source_rows
            || number("pageno")? != 1
            || number("count")? < source_rows
        {
            return Err(protocol(
                "unit page count or first-page identity conflicts with HTML rows",
            ));
        }
    }
    items.truncate(limit as usize);
    Ok(PublicationListing {
        source,
        listing_url: HttpsUrl::new(profile.listing)?,
        response_url: HttpsUrl::new(profile.api.unwrap_or(profile.listing))?,
        observed_at,
        response_sha256: hash,
        source_rows,
        items,
    })
}

pub(crate) fn csrc_listing(
    body: &str,
    limit: u32,
    observed_at: String,
    hash: String,
) -> Result<PublicationListing, OfficialNewsError> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|_| protocol("CSRC listing is not JSON"))?;
    let data = value
        .get("data")
        .ok_or_else(|| protocol("CSRC data envelope is missing"))?;
    if data.get("page").and_then(serde_json::Value::as_u64) != Some(1)
        || data.get("rows").and_then(serde_json::Value::as_u64) != Some(18)
        || data.get("channelId").and_then(serde_json::Value::as_str)
            != Some("a1a078ee0bc54721ab6b148884c784a8")
        || data
            .get("relateSubChannels")
            .and_then(serde_json::Value::as_str)
            != Some("true")
    {
        return Err(protocol("CSRC exact first-page aggregate identity changed"));
    }
    let rows = data
        .get("results")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| protocol("CSRC source results are missing"))?;
    let total = data
        .get("total")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| protocol("CSRC source total is missing"))?;
    if rows.is_empty() || rows.len() != total.min(18) as usize {
        return Err(protocol(
            "CSRC bounded source page is empty or inconsistent",
        ));
    }
    let mut seen = HashSet::new();
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let field = |name: &str| -> Result<&str, OfficialNewsError> {
            row.get(name)
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| protocol("CSRC item field is missing"))
        };
        let url = Url::parse(OfficialSource::Csrc.listing_url())
            .map_err(|_| protocol("CSRC base URL is invalid"))?
            .join(field("url")?)
            .map_err(|_| protocol("CSRC item URL is invalid"))?;
        profiles::validate_article(OfficialSource::Csrc, url.as_str())?;
        if !seen.insert(url.to_string()) {
            return Err(protocol("duplicate CSRC publication URL"));
        }
        let label = field("publishedTimeStr")?;
        let (published_date, precision) = publication_label(label)?;
        validate_observed_date(&published_date, &observed_at)?;
        items.push(PublicationReference {
            source: OfficialSource::Csrc,
            title: checked_text(field("title")?, 512)?,
            canonical_url: HttpsUrl::new(url.to_string())?,
            published_date,
            publication_label: label.to_owned(),
            precision,
            publication_label_origin: PublicationLabelOrigin::ListingApi,
        });
    }
    let source_rows = items.len();
    items.truncate(limit as usize);
    Ok(PublicationListing {
        source: OfficialSource::Csrc,
        listing_url: HttpsUrl::new(OfficialSource::Csrc.listing_url())?,
        response_url: HttpsUrl::new(
            profiles::profile(OfficialSource::Csrc)
                .api
                .ok_or_else(|| protocol("CSRC list API is missing"))?,
        )?,
        observed_at,
        response_sha256: hash,
        source_rows,
        items,
    })
}

pub(crate) fn article(
    source: OfficialSource,
    url: &str,
    html: &str,
    observed_at: String,
    hash: String,
) -> Result<OfficialPublication, OfficialNewsError> {
    let document = Document::parse(html)?;
    let profile = profiles::profile(source);
    let mut metadata = HashMap::new();
    for element in document.select("meta[name][content]")? {
        let name = element
            .attr("name")
            .ok_or_else(|| protocol("meta name is missing"))?
            .to_ascii_lowercase();
        if !["articletitle", "pubdate", "publishdate"].contains(&name.as_str()) {
            continue;
        }
        // These verified templates visibly label publication dates/times.
        // Metadata has different precision or values and must not override them.
        if matches!(
            source,
            OfficialSource::Mof | OfficialSource::Csrc | OfficialSource::Miit | OfficialSource::Nea
        ) && name != "articletitle"
        {
            continue;
        }
        let content = element
            .attr("content")
            .ok_or_else(|| protocol("meta content is missing"))?;
        if metadata.insert(name, content.to_owned()).is_some() {
            return Err(protocol("duplicate article publication metadata"));
        }
    }
    let title = if !profile.title.is_empty() {
        let titles = document.select(profile.title)?;
        if titles.len() != 1 {
            return Err(protocol("article title container is missing or ambiguous"));
        }
        let title = titles[0].text()?;
        if metadata.get("articletitle").is_some_and(|value| {
            value.split_whitespace().collect::<String>()
                != title.split_whitespace().collect::<String>()
        }) {
            return Err(protocol(
                "article title metadata conflicts with visible title",
            ));
        }
        title
    } else {
        metadata
            .get("articletitle")
            .cloned()
            .ok_or_else(|| protocol("ArticleTitle publication metadata is missing"))?
    };
    let title = checked_text(&title, 512)?;
    let visible_contract = match source {
        OfficialSource::Mof => Some((
            ".docreltime > span",
            r"^发布日期：([0-9]{4}年[0-9]{2}月[0-9]{2}日)$",
        )),
        OfficialSource::Csrc => Some((
            ".main > .content > .info > p.fl",
            r"^日期：([0-9]{4}-[0-9]{2}-[0-9]{2}) 来源：(.+)$",
        )),
        OfficialSource::Miit => Some((
            ".cinfo > span#con_time",
            r"^发布时间：([0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2})$",
        )),
        OfficialSource::Nea => Some((
            ".article-title > span.times",
            r"^发布时间：([0-9]{4}-[0-9]{2}-[0-9]{2})$",
        )),
        _ => None,
    };
    let (label, published_date, precision, publication_label_origin) =
        if let Some((selector, pattern)) = visible_contract {
            let lines = document.select(selector)?;
            if lines.len() != 1 {
                return Err(protocol(
                    "visible publication date line is missing or ambiguous",
                ));
            }
            let label = checked_text(&lines[0].text()?, 512)?;
            let pattern = regex::Regex::new(pattern)
                .map_err(|_| protocol("visible date pattern is invalid"))?;
            let captured = pattern
                .captures(&label)
                .ok_or_else(|| protocol("visible publication date line is malformed"))?;
            let numeric = if source == OfficialSource::Mof {
                captured[1].replace(['年', '月'], "-").replace('日', "")
            } else {
                captured[1].to_owned()
            };
            let (date, precision) = publication_label(&numeric)?;
            (
                label,
                date,
                precision,
                PublicationLabelOrigin::VisibleArticleDate,
            )
        } else {
            let labels: Vec<_> = ["pubdate", "publishdate"]
                .into_iter()
                .filter_map(|name| metadata.get(name))
                .collect();
            if labels.len() != 1 {
                return Err(protocol(
                    "publication date metadata is missing or ambiguous",
                ));
            }
            let label = labels[0].trim().to_owned();
            let (date, precision) = publication_label(&label)?;
            (
                label,
                date,
                precision,
                PublicationLabelOrigin::ArticleMetadata,
            )
        };
    validate_observed_date(&published_date, &observed_at)?;
    let containers = document.select(profile.body)?;
    if containers.len() != 1 {
        return Err(protocol(
            "article content container is missing or ambiguous",
        ));
    }
    let content = checked_text(&containers[0].text()?, 1_000_000)?;
    Ok(OfficialPublication {
        source,
        canonical_url: HttpsUrl::new(url)?,
        title,
        published_date,
        publication_label: label,
        precision,
        publication_label_origin,
        content,
        observed_at,
        response_sha256: hash,
    })
}

fn publication_label(label: &str) -> Result<(IsoDate, PublicationPrecision), OfficialNewsError> {
    if !label.is_ascii() {
        return Err(protocol(
            "publication metadata must have an exact numeric date label",
        ));
    }
    let normalized = label.replace('/', "-");
    let precision = match normalized.len() {
        10 => PublicationPrecision::Date,
        16 => PublicationPrecision::Minute,
        19 => PublicationPrecision::Second,
        _ => return Err(protocol("unsupported publication label precision")),
    };
    let date = IsoDate::new(
        normalized
            .get(..10)
            .ok_or_else(|| protocol("publication date is missing"))?,
    )?;
    if precision != PublicationPrecision::Date {
        let bytes = normalized.as_bytes();
        if bytes[10] != b' ' || bytes[13] != b':' || (bytes.len() == 19 && bytes[16] != b':') {
            return Err(protocol("publication time separators are invalid"));
        }
        let part = |start: usize, max: u32| -> Result<(), OfficialNewsError> {
            if !bytes[start..start + 2].iter().all(u8::is_ascii_digit) {
                return Err(protocol(
                    "publication time must contain exact numeric digits",
                ));
            }
            let number = normalized[start..start + 2]
                .parse::<u32>()
                .map_err(|_| protocol("publication time is invalid"))?;
            if number > max {
                return Err(protocol("publication time is outside its range"));
            }
            Ok(())
        };
        part(11, 23)?;
        part(14, 59)?;
        if bytes.len() == 19 {
            part(17, 59)?;
        }
    }
    Ok((date, precision))
}

fn checked_text(value: &str, max: usize) -> Result<String, OfficialNewsError> {
    let text = value.trim();
    if text.is_empty()
        || text.chars().count() > max
        || text
            .chars()
            .any(|ch| ch.is_control() && !ch.is_whitespace())
    {
        return Err(protocol(
            "publication text is empty, oversized or contains invalid controls",
        ));
    }
    Ok(text.to_owned())
}

fn validate_observed_date(date: &IsoDate, observed_at: &str) -> Result<(), OfficialNewsError> {
    // One-day slack covers calendar-date labels in any ordinary time zone.
    // It does not fabricate a time zone or turn the source label into an instant.
    let today =
        time::OffsetDateTime::parse(observed_at, &time::format_description::well_known::Rfc3339)
            .map_err(|_| protocol("observation timestamp is invalid"))?
            .date();
    let latest = today
        .next_day()
        .ok_or_else(|| protocol("observation date is outside its range"))?;
    let format = time::format_description::parse_borrowed::<3>("[year]-[month]-[day]")
        .map_err(|_| protocol("observation date format is invalid"))?;
    let maximum = latest
        .format(&format)
        .map_err(|_| protocol("observation date cannot be formatted"))?;
    if date.as_str() > maximum.as_str() {
        return Err(protocol(
            "publication date is in the future relative to observation",
        ));
    }
    Ok(())
}
