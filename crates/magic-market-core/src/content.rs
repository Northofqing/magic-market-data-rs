use crate::{
    DataBatch, Exchange, HttpsUrl, InstrumentDateRangeRequest, InstrumentId, IsoDate, NonEmptyText,
    PositiveU32, SourceEvidence, SourcedRecord,
};
use serde::{de, Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsItem {
    pub item_id: NonEmptyText,
    pub title: NonEmptyText,
    pub summary: Option<NonEmptyText>,
    pub content: Option<NonEmptyText>,
    pub publisher: NonEmptyText,
    pub canonical_url: HttpsUrl,
    pub published_at: NonEmptyText,
    pub instruments: Vec<InstrumentId>,
    pub topics: Vec<NonEmptyText>,
    pub language: NonEmptyText,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Announcement {
    pub announcement_id: NonEmptyText,
    pub instrument: InstrumentId,
    #[serde(default)]
    pub instrument_name: Option<NonEmptyText>,
    pub category: Option<NonEmptyText>,
    pub title: NonEmptyText,
    pub published_at: NonEmptyText,
    pub canonical_url: HttpsUrl,
    pub pdf_url: Option<HttpsUrl>,
    pub evidence: SourceEvidence,
}

/// One public investor-interaction question and its optional answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InvestorQuestion {
    question_id: NonEmptyText,
    instrument: InstrumentId,
    company: NonEmptyText,
    question: NonEmptyText,
    question_at: NonEmptyText,
    answer: Option<NonEmptyText>,
    answer_at: Option<NonEmptyText>,
    source_question_id: Option<NonEmptyText>,
    answerer: Option<NonEmptyText>,
    evidence: SourceEvidence,
}

impl InvestorQuestion {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        question_id: NonEmptyText,
        instrument: InstrumentId,
        company: NonEmptyText,
        question: NonEmptyText,
        question_at: NonEmptyText,
        answer: Option<NonEmptyText>,
        answer_at: Option<NonEmptyText>,
        evidence: SourceEvidence,
    ) -> Result<Self, crate::CoreError> {
        Self::new_with_metadata(
            question_id,
            instrument,
            company,
            question,
            question_at,
            answer,
            answer_at,
            None,
            None,
            evidence,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_metadata(
        question_id: NonEmptyText,
        instrument: InstrumentId,
        company: NonEmptyText,
        question: NonEmptyText,
        question_at: NonEmptyText,
        answer: Option<NonEmptyText>,
        answer_at: Option<NonEmptyText>,
        source_question_id: Option<NonEmptyText>,
        answerer: Option<NonEmptyText>,
        evidence: SourceEvidence,
    ) -> Result<Self, crate::CoreError> {
        if answer.is_none() && answer_at.is_some() {
            return Err(crate::CoreError::InvalidRequest(
                "answer_at cannot be present without an answer".into(),
            ));
        }
        if answer.is_none() && answerer.is_some() {
            return Err(crate::CoreError::InvalidRequest(
                "answerer cannot be present without an answer".into(),
            ));
        }
        Ok(Self {
            question_id,
            instrument,
            company,
            question,
            question_at,
            answer,
            answer_at,
            source_question_id,
            answerer,
            evidence,
        })
    }

    pub fn question_id(&self) -> &NonEmptyText {
        &self.question_id
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn company(&self) -> &NonEmptyText {
        &self.company
    }

    pub fn question(&self) -> &NonEmptyText {
        &self.question
    }

    pub fn question_at(&self) -> &NonEmptyText {
        &self.question_at
    }

    pub fn answer(&self) -> Option<&NonEmptyText> {
        self.answer.as_ref()
    }

    pub fn answer_at(&self) -> Option<&NonEmptyText> {
        self.answer_at.as_ref()
    }

    pub fn source_question_id(&self) -> Option<&NonEmptyText> {
        self.source_question_id.as_ref()
    }

    pub fn answerer(&self) -> Option<&NonEmptyText> {
        self.answerer.as_ref()
    }

    pub fn evidence(&self) -> &SourceEvidence {
        &self.evidence
    }
}

#[derive(Deserialize)]
struct InvestorQuestionWire {
    question_id: NonEmptyText,
    instrument: InstrumentId,
    company: NonEmptyText,
    question: NonEmptyText,
    question_at: NonEmptyText,
    answer: Option<NonEmptyText>,
    answer_at: Option<NonEmptyText>,
    source_question_id: Option<NonEmptyText>,
    answerer: Option<NonEmptyText>,
    evidence: SourceEvidence,
}

impl<'de> Deserialize<'de> for InvestorQuestion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = InvestorQuestionWire::deserialize(deserializer)?;
        Self::new_with_metadata(
            wire.question_id,
            wire.instrument,
            wire.company,
            wire.question,
            wire.question_at,
            wire.answer,
            wire.answer_at,
            wire.source_question_id,
            wire.answerer,
            wire.evidence,
        )
        .map_err(de::Error::custom)
    }
}

macro_rules! impl_sourced {
    ($($record:ty),+ $(,)?) => {
        $(
            impl SourcedRecord for $record {
                fn provider_id(&self) -> crate::ProviderId {
                    self.evidence.provider()
                }

                fn evidence_batch_id(&self) -> &str {
                    self.evidence.batch_id()
                }
            }
        )+
    };
}

impl_sourced!(NewsItem, Announcement, InvestorQuestion);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ContentCapabilities {
    pub instrument_news: bool,
    pub global_news: bool,
    pub announcements: bool,
    pub announcement_discovery: bool,
    pub investor_questions: bool,
}

/// Bounded full-market announcement request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnnouncementDiscoveryRequest {
    start: IsoDate,
    end: IsoDate,
    exchange: Option<Exchange>,
    limit: PositiveU32,
}

impl AnnouncementDiscoveryRequest {
    pub fn new(start: IsoDate, end: IsoDate, limit: PositiveU32) -> Result<Self, crate::CoreError> {
        if start > end {
            return Err(crate::CoreError::InvalidRequest(
                "announcement discovery start must not exceed end".into(),
            ));
        }
        if limit.get() > 10_000 {
            return Err(crate::CoreError::InvalidRequest(
                "announcement discovery limit must be at most 10000".into(),
            ));
        }
        Ok(Self {
            start,
            end,
            exchange: None,
            limit,
        })
    }

    pub fn with_exchange(mut self, exchange: Exchange) -> Self {
        self.exchange = Some(exchange);
        self
    }

    pub fn start(&self) -> &IsoDate {
        &self.start
    }

    pub fn end(&self) -> &IsoDate {
        &self.end
    }

    pub fn exchange(&self) -> Option<Exchange> {
        self.exchange
    }

    pub fn limit(&self) -> PositiveU32 {
        self.limit
    }
}

impl<'de> Deserialize<'de> for AnnouncementDiscoveryRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            start: IsoDate,
            end: IsoDate,
            exchange: Option<Exchange>,
            limit: PositiveU32,
        }
        let wire = Wire::deserialize(deserializer)?;
        let mut request = Self::new(wire.start, wire.end, wire.limit).map_err(de::Error::custom)?;
        if let Some(exchange) = wire.exchange {
            request = request.with_exchange(exchange);
        }
        Ok(request)
    }
}

pub trait NewsProvider {
    type Error: std::error::Error + Send + Sync + 'static;
    fn instrument_news(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<NewsItem>, Self::Error>;
    fn global_news(&self, limit: PositiveU32) -> Result<DataBatch<NewsItem>, Self::Error>;
}

pub trait Announcements {
    type Error: std::error::Error + Send + Sync + 'static;
    fn announcements(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<Announcement>, Self::Error>;
}

pub trait AnnouncementDiscovery {
    type Error: std::error::Error + Send + Sync + 'static;

    fn discover_announcements(
        &self,
        request: &AnnouncementDiscoveryRequest,
    ) -> Result<DataBatch<Announcement>, Self::Error>;
}

pub trait InvestorQuestions {
    type Error: std::error::Error + Send + Sync + 'static;
    fn investor_questions(
        &self,
        request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<InvestorQuestion>, Self::Error>;
}
