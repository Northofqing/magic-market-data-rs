//! Bounded candidate discovery over already admitted content operations.
//!
//! A match is not a source-published topic or verified disclosure event. The
//! original canonical payload and its evidence remain attached to every hit.

use magic_market_core::{
    InstrumentDateRangeRequest, InstrumentId, IsoDate, MarketAnnouncementRequest, PositiveU32,
};
use magic_market_service::{
    BlockingQueryGateway, CanonicalPayload, Operation, QueryCommand, ServiceError,
};
use serde_json::{json, Value};

use crate::grpc_production::{
    ANNOUNCEMENTS_RECORD_SCHEMA, ANNOUNCEMENTS_REQUEST_SCHEMA, GLOBAL_NEWS_RECORD_SCHEMA,
    GLOBAL_NEWS_REQUEST_SCHEMA, MARKET_ANNOUNCEMENTS_REQUEST_SCHEMA, NEWS_SCHEMA_VERSION,
    SCHEMA_VERSION,
};

const NEWS_SOURCE_LIMIT: u32 = 20;
const DISCLOSURE_SOURCE_LIMIT: u32 = 300;
const INSTRUMENT_DISCLOSURE_SOURCE_LIMIT: u32 = 200;
const NEWS_SOURCES: [&str; 8] = [
    "WallstreetCn",
    "Cailianpress",
    "ThePaper",
    "XinhuaFinance",
    "Yicai",
    "Jin10",
    "Yonhap",
    "Eastmoney",
];

/// Conjunction of entity groups; each group contains alternative spellings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityQuery {
    groups: Vec<Vec<String>>,
}

impl EntityQuery {
    pub fn new(groups: Vec<Vec<String>>) -> Result<Self, ServiceError> {
        if groups.is_empty() || groups.len() > 4 {
            return Err(ServiceError::InvalidRequest(
                "news query requires one through four entity groups".into(),
            ));
        }
        let mut normalized = Vec::with_capacity(groups.len());
        for group in groups {
            if group.is_empty() || group.len() > 8 {
                return Err(ServiceError::InvalidRequest(
                    "each entity group requires one through eight aliases".into(),
                ));
            }
            let mut aliases = Vec::with_capacity(group.len());
            for alias in group {
                let alias = alias.trim();
                if alias.is_empty() || alias.chars().count() > 80 {
                    return Err(ServiceError::InvalidRequest(
                        "news alias must contain one through 80 characters".into(),
                    ));
                }
                aliases.push(alias.to_lowercase());
            }
            normalized.push(aliases);
        }
        Ok(Self { groups: normalized })
    }

    fn matches(&self, document: &Value) -> bool {
        let text = ["title", "summary", "content"]
            .into_iter()
            .filter_map(|field| document.get(field).and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        self.groups.iter().all(|group| {
            group
                .iter()
                .any(|alias| contains_bounded_alias(&text, alias))
        })
    }
}

fn contains_bounded_alias(text: &str, alias: &str) -> bool {
    text.match_indices(alias).any(|(start, _)| {
        let before = text[..start].chars().next_back();
        let after = text[start + alias.len()..].chars().next();
        (!alias
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphanumeric())
            || !before.is_some_and(|ch| ch.is_ascii_alphanumeric()))
            && (!alias
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_ascii_alphanumeric())
                || !after.is_some_and(|ch| ch.is_ascii_alphanumeric()))
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisclosureKind {
    AnnualReport,
    HalfYearReport,
    ShareholderReduction,
    RepurchasedShareReduction,
    EarningsForecast,
    ShareholderIncrease,
    EquityIssuance,
    ControlChange,
    ShareTransfer,
    SharePledge,
    ShareFreeze,
    ShareRepurchase,
    EquityIncentive,
    EmployeeOwnership,
}

/// An explicit source window, not a claim about all historical documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryScope {
    LatestPerSource {
        limit: u32,
    },
    AnnouncementRange {
        start: IsoDate,
        end: IsoDate,
        source_limit: u32,
    },
    InstrumentAnnouncementRange {
        instrument: InstrumentId,
        start: IsoDate,
        end: IsoDate,
        source_limit: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceOutcome {
    Inspected {
        provider: String,
        records: usize,
        /// Source-batch quality, never exhaustive history or market coverage.
        source_complete: bool,
    },
    Failed {
        provider: String,
        error: ServiceError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryStatus {
    /// Every attempted source window was inspected; see its source quality flag.
    AllSourcesInspected,
    PartialSourceFailure,
    NoSourceSucceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryPage<T> {
    pub scope: DiscoveryScope,
    pub status: DiscoveryStatus,
    pub sources: Vec<SourceOutcome>,
    pub candidates: Vec<T>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsCandidate {
    pub provider: String,
    pub title: String,
    pub url: String,
    pub record: CanonicalPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisclosureCandidate {
    pub kind: DisclosureKind,
    pub title: String,
    pub url: String,
    pub record: CanonicalPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisclosureQuery {
    start: IsoDate,
    end: IsoDate,
    kind: DisclosureKind,
    instrument: Option<InstrumentId>,
}

impl DisclosureQuery {
    pub fn market_day(date: IsoDate, kind: DisclosureKind) -> Result<Self, ServiceError> {
        MarketAnnouncementRequest::new(
            date.clone(),
            date.clone(),
            positive(DISCLOSURE_SOURCE_LIMIT)?,
        )
        .map_err(|error| ServiceError::InvalidRequest(error.to_string()))?;
        Ok(Self {
            start: date.clone(),
            end: date,
            kind,
            instrument: None,
        })
    }

    pub fn instrument_range(
        instrument: InstrumentId,
        start: IsoDate,
        end: IsoDate,
        kind: DisclosureKind,
    ) -> Result<Self, ServiceError> {
        InstrumentDateRangeRequest::new(
            instrument.clone(),
            positive(INSTRUMENT_DISCLOSURE_SOURCE_LIMIT)?,
        )
        .and_then(|request| request.with_range(start.clone(), end.clone()))
        .map_err(|error| ServiceError::InvalidRequest(error.to_string()))?;
        Ok(Self {
            start,
            end,
            kind,
            instrument: Some(instrument),
        })
    }
}

/// Search only the current bounded windows of admitted GlobalNews providers.
pub fn search_recent_news<G: BlockingQueryGateway>(
    gateway: &G,
    query: &EntityQuery,
) -> Result<DiscoveryPage<NewsCandidate>, ServiceError> {
    let available = gateway.capabilities();
    let mut page = DiscoveryPage {
        scope: DiscoveryScope::LatestPerSource {
            limit: NEWS_SOURCE_LIMIT,
        },
        status: DiscoveryStatus::NoSourceSucceeded,
        sources: Vec::new(),
        candidates: Vec::new(),
    };
    for provider in NEWS_SOURCES {
        if !available.iter().any(|capability| {
            capability.operation == Operation::GlobalNews
                && capability.provider == provider
                && capability.repository_admitted
                && capability.runtime_available
        }) {
            continue;
        }
        let command = command(
            Operation::GlobalNews,
            provider,
            GLOBAL_NEWS_REQUEST_SCHEMA,
            NEWS_SCHEMA_VERSION,
            json!({ "limit": NEWS_SOURCE_LIMIT }),
        )?;
        match gateway.execute(command) {
            Ok(result) => {
                let parsed = (|| -> Result<Vec<NewsCandidate>, ServiceError> {
                    if result.provider != provider
                        || !result.repository_admitted
                        || !result.complete
                        || result.records.len() > NEWS_SOURCE_LIMIT as usize
                    {
                        return Err(ServiceError::FailedPrecondition(
                            "news source result lacks admitted complete provider identity".into(),
                        ));
                    }
                    let mut candidates = Vec::new();
                    for record in &result.records {
                        let value =
                            parse_record(record, GLOBAL_NEWS_RECORD_SCHEMA, NEWS_SCHEMA_VERSION)?;
                        let title = required_text(&value, "title")?;
                        let url = required_text(&value, "url")?;
                        if query.matches(&value) {
                            candidates.push(NewsCandidate {
                                provider: provider.to_owned(),
                                title: title.to_owned(),
                                url: url.to_owned(),
                                record: record.clone(),
                            });
                        }
                    }
                    Ok(candidates)
                })();
                match parsed {
                    Ok(mut candidates) => {
                        page.sources.push(SourceOutcome::Inspected {
                            provider: provider.to_owned(),
                            records: result.records.len(),
                            source_complete: result.complete,
                        });
                        page.candidates.append(&mut candidates);
                    }
                    Err(error) => page.sources.push(SourceOutcome::Failed {
                        provider: provider.to_owned(),
                        error,
                    }),
                }
            }
            Err(error) => page.sources.push(SourceOutcome::Failed {
                provider: provider.to_owned(),
                error,
            }),
        }
    }
    if page.sources.is_empty() {
        return Err(ServiceError::Unsupported {
            operation: Operation::GlobalNews,
            reason: "no admitted runtime-available news source".into(),
        });
    }
    let successful = page
        .sources
        .iter()
        .filter(|outcome| matches!(outcome, SourceOutcome::Inspected { .. }))
        .count();
    page.status = if successful == 0 {
        DiscoveryStatus::NoSourceSucceeded
    } else if successful == page.sources.len() {
        DiscoveryStatus::AllSourcesInspected
    } else {
        DiscoveryStatus::PartialSourceFailure
    };
    Ok(page)
}

/// Select disclosure candidates from the bounded Cninfo market-announcement range.
pub fn discover_disclosures<G: BlockingQueryGateway>(
    gateway: &G,
    query: &DisclosureQuery,
) -> Result<DiscoveryPage<DisclosureCandidate>, ServiceError> {
    let (operation, schema, body, scope) = if let Some(instrument) = &query.instrument {
        let request = InstrumentDateRangeRequest::new(
            instrument.clone(),
            positive(INSTRUMENT_DISCLOSURE_SOURCE_LIMIT)?,
        )
        .and_then(|request| request.with_range(query.start.clone(), query.end.clone()))
        .map_err(|error| ServiceError::InvalidRequest(error.to_string()))?;
        (
            Operation::Announcements,
            ANNOUNCEMENTS_REQUEST_SCHEMA,
            serde_json::to_value(request)
                .map_err(|error| ServiceError::Internal(error.to_string()))?,
            DiscoveryScope::InstrumentAnnouncementRange {
                instrument: instrument.clone(),
                start: query.start.clone(),
                end: query.end.clone(),
                source_limit: INSTRUMENT_DISCLOSURE_SOURCE_LIMIT,
            },
        )
    } else {
        let request = MarketAnnouncementRequest::new(
            query.start.clone(),
            query.end.clone(),
            positive(DISCLOSURE_SOURCE_LIMIT)?,
        )
        .map_err(|error| ServiceError::InvalidRequest(error.to_string()))?;
        (
            Operation::MarketAnnouncements,
            MARKET_ANNOUNCEMENTS_REQUEST_SCHEMA,
            serde_json::to_value(request)
                .map_err(|error| ServiceError::Internal(error.to_string()))?,
            DiscoveryScope::AnnouncementRange {
                start: query.start.clone(),
                end: query.end.clone(),
                source_limit: DISCLOSURE_SOURCE_LIMIT,
            },
        )
    };
    let command = command(operation, "Cninfo", schema, SCHEMA_VERSION, body)?;
    let result = gateway.execute(command)?;
    let inspected_limit = if query.instrument.is_some() {
        INSTRUMENT_DISCLOSURE_SOURCE_LIMIT
    } else {
        DISCLOSURE_SOURCE_LIMIT
    };
    if result.provider != "Cninfo"
        || !result.repository_admitted
        || result.records.len() > inspected_limit as usize
    {
        return Err(ServiceError::FailedPrecondition(
            "announcement result lacks admitted bounded Cninfo identity".into(),
        ));
    }
    let mut candidates = Vec::new();
    for record in &result.records {
        let value = parse_record(record, ANNOUNCEMENTS_RECORD_SCHEMA, SCHEMA_VERSION)?;
        let title = required_text(&value, "title")?;
        if classify_disclosure(title) == Some(query.kind) {
            candidates.push(DisclosureCandidate {
                kind: query.kind,
                title: title.to_owned(),
                url: required_text(&value, "canonical_url")?.to_owned(),
                record: record.clone(),
            });
        }
    }
    Ok(DiscoveryPage {
        scope,
        status: DiscoveryStatus::AllSourcesInspected,
        sources: vec![SourceOutcome::Inspected {
            provider: "Cninfo".into(),
            records: result.records.len(),
            source_complete: result.complete,
        }],
        candidates,
    })
}

fn classify_disclosure(title: &str) -> Option<DisclosureKind> {
    // A title can mention several actions (for example, a pledge by a
    // controlling shareholder). Return only one unambiguous candidate kind;
    // never turn the wording into evidence of an executed transaction.
    let shareholder = ["股东", "董监高", "董事", "高管", "实际控制人", "控股股东"]
        .iter()
        .any(|word| title.contains(word));
    let equity = title.contains("股份") || title.contains("股票") || title.contains("股权");
    let half_year_report = title.contains("半年度报告") || title.contains("半年报");
    let non_half_year_title = title.replace("半年度报告", "").replace("半年报", "");
    let repurchased_share_reduction = title.contains("回购股份") && title.contains("减持");
    let mut matches = Vec::new();
    let mut consider = |condition, kind| {
        if condition {
            matches.push(kind);
        }
    };
    consider(half_year_report, DisclosureKind::HalfYearReport);
    consider(
        non_half_year_title.contains("年度报告") || non_half_year_title.contains("年报"),
        DisclosureKind::AnnualReport,
    );
    consider(title.contains("业绩预告"), DisclosureKind::EarningsForecast);
    consider(
        repurchased_share_reduction,
        DisclosureKind::RepurchasedShareReduction,
    );
    consider(
        title.contains("减持") && shareholder && !repurchased_share_reduction,
        DisclosureKind::ShareholderReduction,
    );
    consider(
        title.contains("增持") && shareholder && equity,
        DisclosureKind::ShareholderIncrease,
    );
    consider(
        (title.contains("向特定对象发行") && equity)
            || title.contains("非公开发行股票")
            || title.contains("定向增发")
            || title.contains("增发股票")
            || (title.contains("配股") && !title.contains("分配股"))
            || title.contains("发行股份购买资产"),
        DisclosureKind::EquityIssuance,
    );
    consider(
        (title.contains("控制权变更")
            || title.contains("实际控制人变更")
            || title.contains("控股股东变更"))
            && ![
                "未发生变更",
                "不发生变更",
                "不会发生变更",
                "未发生变化",
                "不发生变化",
                "不会发生变化",
                "不涉及",
                "未变更",
                "不会导致",
                "不导致",
                "不会引起",
                "不引起",
            ]
            .iter()
            .any(|word| title.contains(word)),
        DisclosureKind::ControlChange,
    );
    consider(
        equity && (title.contains("协议转让") || title.contains("无偿划转")),
        DisclosureKind::ShareTransfer,
    );
    consider(
        equity && title.contains("质押"),
        DisclosureKind::SharePledge,
    );
    consider(
        equity && (title.contains("冻结") || title.contains("解冻")),
        DisclosureKind::ShareFreeze,
    );
    consider(
        (title.contains("回购股份") || title.contains("回购公司股份"))
            && !title.contains("注销")
            && !title.contains("减持"),
        DisclosureKind::ShareRepurchase,
    );
    consider(
        title.contains("股权激励")
            || title.contains("限制性股票激励")
            || title.contains("股票期权激励"),
        DisclosureKind::EquityIncentive,
    );
    consider(
        title.contains("员工持股"),
        DisclosureKind::EmployeeOwnership,
    );
    (matches.len() == 1).then(|| matches[0])
}

fn parse_record(
    record: &CanonicalPayload,
    schema: &str,
    version: u32,
) -> Result<Value, ServiceError> {
    if record.schema() != schema || record.schema_version() != version {
        return Err(ServiceError::FailedPrecondition(
            "content record schema does not match the admitted contract".into(),
        ));
    }
    serde_json::from_slice(record.data())
        .map_err(|_| ServiceError::FailedPrecondition("content record JSON is invalid".into()))
}

fn required_text<'a>(value: &'a Value, field: &str) -> Result<&'a str, ServiceError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ServiceError::FailedPrecondition(format!("content record {field} is missing"))
        })
}

fn positive(value: u32) -> Result<PositiveU32, ServiceError> {
    PositiveU32::new(value).map_err(|error| ServiceError::Internal(error.to_string()))
}

fn command(
    operation: Operation,
    provider: &str,
    schema: &str,
    version: u32,
    body: Value,
) -> Result<QueryCommand, ServiceError> {
    let bytes =
        serde_json::to_vec(&body).map_err(|error| ServiceError::Internal(error.to_string()))?;
    let payload = CanonicalPayload::new(schema, version, bytes, 4096)?;
    QueryCommand::new(
        "bounded-content-discovery",
        operation,
        Some(provider.to_owned()),
        payload,
    )
}

#[cfg(test)]
#[path = "../tests/internal/content_discovery_tests.rs"]
mod tests;
