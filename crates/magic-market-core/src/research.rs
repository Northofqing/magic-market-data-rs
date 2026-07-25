use crate::{
    DataBatch, FiniteNumber, HttpsUrl, InstrumentId, Money, NonEmptyText, PositiveU32,
    SourceEvidence, SourcedRecord,
};
use serde::{de, Deserialize, Deserializer, Serialize};

/// Scope used by research-report queries and records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportScope {
    Instrument(InstrumentId),
    Industry(NonEmptyText),
}

/// One fiscal-period estimate retained exactly as supplied.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EarningsEstimate {
    fiscal_year: PositiveU32,
    eps: Option<FiniteNumber>,
    eps_min: Option<FiniteNumber>,
    eps_max: Option<FiniteNumber>,
    contributor_count: Option<PositiveU32>,
    revenue: Option<Money>,
    profit: Option<Money>,
}

impl EarningsEstimate {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        fiscal_year: PositiveU32,
        eps: Option<FiniteNumber>,
        eps_min: Option<FiniteNumber>,
        eps_max: Option<FiniteNumber>,
        contributor_count: Option<PositiveU32>,
        revenue: Option<Money>,
        profit: Option<Money>,
    ) -> Result<Self, crate::CoreError> {
        if let (Some(minimum), Some(maximum)) = (eps_min, eps_max) {
            if minimum.get() > maximum.get() {
                return Err(crate::CoreError::InvalidRequest(
                    "earnings EPS minimum must not exceed maximum".into(),
                ));
            }
        }
        Ok(Self {
            fiscal_year,
            eps,
            eps_min,
            eps_max,
            contributor_count,
            revenue,
            profit,
        })
    }

    pub fn fiscal_year(&self) -> PositiveU32 {
        self.fiscal_year
    }

    pub fn eps(&self) -> Option<FiniteNumber> {
        self.eps
    }

    pub fn eps_min(&self) -> Option<FiniteNumber> {
        self.eps_min
    }

    pub fn eps_max(&self) -> Option<FiniteNumber> {
        self.eps_max
    }

    pub fn contributor_count(&self) -> Option<PositiveU32> {
        self.contributor_count
    }

    pub fn revenue(&self) -> Option<Money> {
        self.revenue
    }

    pub fn profit(&self) -> Option<Money> {
        self.profit
    }
}

#[derive(Deserialize)]
struct EarningsEstimateWire {
    fiscal_year: PositiveU32,
    eps: Option<FiniteNumber>,
    eps_min: Option<FiniteNumber>,
    eps_max: Option<FiniteNumber>,
    contributor_count: Option<PositiveU32>,
    revenue: Option<Money>,
    profit: Option<Money>,
}

impl<'de> Deserialize<'de> for EarningsEstimate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = EarningsEstimateWire::deserialize(deserializer)?;
        Self::new(
            wire.fiscal_year,
            wire.eps,
            wire.eps_min,
            wire.eps_max,
            wire.contributor_count,
            wire.revenue,
            wire.profit,
        )
        .map_err(de::Error::custom)
    }
}

/// One published stock or industry research report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchReport {
    pub report_id: NonEmptyText,
    pub scope: ReportScope,
    pub title: NonEmptyText,
    pub organization: NonEmptyText,
    pub author: Option<NonEmptyText>,
    pub rating: Option<NonEmptyText>,
    pub industry_code: Option<NonEmptyText>,
    pub industry_name: Option<NonEmptyText>,
    pub published_at: NonEmptyText,
    pub canonical_url: HttpsUrl,
    pub pdf_url: Option<HttpsUrl>,
    pub estimates: Vec<EarningsEstimate>,
    pub evidence: SourceEvidence,
}

impl SourcedRecord for ResearchReport {
    fn provider_id(&self) -> crate::ProviderId {
        self.evidence.provider()
    }

    fn evidence_batch_id(&self) -> &str {
        self.evidence.batch_id()
    }
}

/// Provider consensus for one instrument and observation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsensusSnapshot {
    pub instrument: InstrumentId,
    pub estimates: Vec<EarningsEstimate>,
    pub contributor_count: Option<PositiveU32>,
    pub evidence: SourceEvidence,
}

impl SourcedRecord for ConsensusSnapshot {
    fn provider_id(&self) -> crate::ProviderId {
        self.evidence.provider()
    }

    fn evidence_batch_id(&self) -> &str {
        self.evidence.batch_id()
    }
}

/// Searchable content family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticChannel {
    Report,
    News,
    Announcement,
    General,
}

/// Normalized semantic-search document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticSearchDocument {
    pub document_id: NonEmptyText,
    pub channel: SemanticChannel,
    pub title: NonEmptyText,
    pub excerpt: Option<NonEmptyText>,
    pub canonical_url: HttpsUrl,
    pub published_at: Option<NonEmptyText>,
    pub evidence: SourceEvidence,
}

impl SourcedRecord for SemanticSearchDocument {
    fn provider_id(&self) -> crate::ProviderId {
        self.evidence.provider()
    }

    fn evidence_batch_id(&self) -> &str {
        self.evidence.batch_id()
    }
}

/// Bounded report page request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResearchRequest {
    scope: ReportScope,
    page: PositiveU32,
    page_size: PositiveU32,
}

impl ResearchRequest {
    pub fn new(
        scope: ReportScope,
        page: PositiveU32,
        page_size: PositiveU32,
    ) -> Result<Self, crate::CoreError> {
        if page_size.get() > 100 {
            return Err(crate::CoreError::InvalidRequest(
                "research page_size must be at most 100".into(),
            ));
        }
        Ok(Self {
            scope,
            page,
            page_size,
        })
    }

    pub fn scope(&self) -> &ReportScope {
        &self.scope
    }

    pub fn page(&self) -> PositiveU32 {
        self.page
    }

    pub fn page_size(&self) -> PositiveU32 {
        self.page_size
    }
}

#[derive(Deserialize)]
struct ResearchRequestWire {
    scope: ReportScope,
    page: PositiveU32,
    page_size: PositiveU32,
}

impl<'de> Deserialize<'de> for ResearchRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ResearchRequestWire::deserialize(deserializer)?;
        Self::new(wire.scope, wire.page, wire.page_size).map_err(de::Error::custom)
    }
}

/// Bounded semantic query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticSearchRequest {
    query: NonEmptyText,
    channel: SemanticChannel,
    limit: PositiveU32,
}

impl SemanticSearchRequest {
    pub fn new(
        query: impl Into<String>,
        channel: SemanticChannel,
        limit: PositiveU32,
    ) -> Result<Self, crate::CoreError> {
        if limit.get() > 100 {
            return Err(crate::CoreError::InvalidRequest(
                "semantic search limit must be at most 100".into(),
            ));
        }
        Ok(Self {
            query: NonEmptyText::new(query)?,
            channel,
            limit,
        })
    }

    pub fn query(&self) -> &NonEmptyText {
        &self.query
    }

    pub fn channel(&self) -> SemanticChannel {
        self.channel
    }

    pub fn limit(&self) -> PositiveU32 {
        self.limit
    }
}

#[derive(Deserialize)]
struct SemanticSearchRequestWire {
    query: String,
    channel: SemanticChannel,
    limit: PositiveU32,
}

impl<'de> Deserialize<'de> for SemanticSearchRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SemanticSearchRequestWire::deserialize(deserializer)?;
        Self::new(wire.query, wire.channel, wire.limit).map_err(de::Error::custom)
    }
}

/// Research-domain capability declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ResearchCapabilities {
    pub reports: bool,
    pub consensus: bool,
    pub semantic_search: bool,
    pub pdf_download: bool,
    pub document_body: bool,
}

/// Identity pair for one source report document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchDocumentRequest {
    pub report_id: NonEmptyText,
    pub pdf_url: HttpsUrl,
}

/// Original bounded report body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResearchDocument {
    pub report_id: NonEmptyText,
    pub pdf_url: HttpsUrl,
    pub content_type: NonEmptyText,
    pub body: Vec<u8>,
    pub evidence: SourceEvidence,
}

impl ResearchDocument {
    pub fn new(
        report_id: NonEmptyText,
        pdf_url: HttpsUrl,
        body: Vec<u8>,
        evidence: SourceEvidence,
    ) -> Result<Self, crate::CoreError> {
        if body.len() < 8 || !body.starts_with(b"%PDF-") {
            return Err(crate::CoreError::InvalidRequest(
                "research document body must start with a PDF header".into(),
            ));
        }
        if body.len() > 32 * 1024 * 1024 {
            return Err(crate::CoreError::InvalidRequest(
                "research document body must be at most 32 MiB".into(),
            ));
        }
        Ok(Self {
            report_id,
            pdf_url,
            content_type: NonEmptyText::new("application/pdf")?,
            body,
            evidence,
        })
    }
}

#[derive(Deserialize)]
struct ResearchDocumentWire {
    report_id: NonEmptyText,
    pdf_url: HttpsUrl,
    content_type: NonEmptyText,
    body: Vec<u8>,
    evidence: SourceEvidence,
}

impl<'de> Deserialize<'de> for ResearchDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ResearchDocumentWire::deserialize(deserializer)?;
        if wire.content_type.as_str() != "application/pdf" {
            return Err(de::Error::custom(
                "research document content_type must be application/pdf",
            ));
        }
        Self::new(wire.report_id, wire.pdf_url, wire.body, wire.evidence).map_err(de::Error::custom)
    }
}

impl SourcedRecord for ResearchDocument {
    fn provider_id(&self) -> crate::ProviderId {
        self.evidence.provider()
    }

    fn evidence_batch_id(&self) -> &str {
        self.evidence.batch_id()
    }
}

pub trait ResearchReports {
    type Error: std::error::Error + Send + Sync + 'static;

    fn research_reports(
        &self,
        request: &ResearchRequest,
    ) -> Result<DataBatch<ResearchReport>, Self::Error>;
}

pub trait ResearchDocuments {
    type Error: std::error::Error + Send + Sync + 'static;

    fn research_document(
        &self,
        request: &ResearchDocumentRequest,
    ) -> Result<DataBatch<ResearchDocument>, Self::Error>;
}

pub trait ConsensusData {
    type Error: std::error::Error + Send + Sync + 'static;

    fn consensus(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<ConsensusSnapshot>, Self::Error>;
}

pub trait SemanticSearch {
    type Error: std::error::Error + Send + Sync + 'static;

    fn semantic_search(
        &self,
        request: &SemanticSearchRequest,
    ) -> Result<DataBatch<SemanticSearchDocument>, Self::Error>;
}
